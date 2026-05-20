//! Feature-gated local model Steward boundary.

use chrono::{DateTime, Utc};
use continuitydb_core::{SemanticAnchor, StateCellId};
use continuitydb_revision::RevisionLinkKind;
use serde::Deserialize;

use crate::{ProposalId, StewardAction, StewardError, StewardIdentity, StewardProposal};

/// Backend that runs local model inference for the database Steward.
pub trait LocalModelBackend {
    /// Runs inference for a prompt and returns a JSON proposal response.
    fn infer(&self, request: LocalModelRequest) -> Result<String, StewardError>;
}

/// Request sent to a local model backend.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalModelRequest {
    task: String,
    evidence: Vec<LocalModelEvidence>,
    prompt: String,
}

impl LocalModelRequest {
    fn from_input(input: &LocalModelStewardInput) -> Self {
        let prompt = build_prompt(&input.task, &input.evidence);
        Self {
            task: input.task.clone(),
            evidence: input.evidence.clone(),
            prompt,
        }
    }

    /// Returns the Steward task.
    pub fn task(&self) -> &str {
        &self.task
    }

    /// Returns evidence snippets available to the local model.
    pub fn evidence(&self) -> &[LocalModelEvidence] {
        &self.evidence
    }

    /// Returns the deterministic prompt sent to the local model.
    pub fn prompt(&self) -> &str {
        &self.prompt
    }
}

/// Evidence snippet supplied to a local model Steward.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalModelEvidence {
    locator: String,
    text: String,
}

impl LocalModelEvidence {
    /// Returns the citation locator for this evidence snippet.
    pub fn locator(&self) -> &str {
        &self.locator
    }

    /// Returns the evidence text.
    pub fn text(&self) -> &str {
        &self.text
    }
}

/// Feature-gated Steward that asks a local model backend for proposals.
#[derive(Clone, Debug)]
pub struct LocalModelSteward<B> {
    identity: StewardIdentity,
    backend: B,
}

impl<B> LocalModelSteward<B>
where
    B: LocalModelBackend,
{
    /// Creates a local model Steward with a stable identity and backend.
    pub fn new(identity: StewardIdentity, backend: B) -> Self {
        Self { identity, backend }
    }

    /// Returns the configured backend.
    pub fn backend(&self) -> &B {
        &self.backend
    }

    /// Runs local model inference and decodes validated Steward proposals.
    pub fn propose(
        &self,
        input: LocalModelStewardInput,
    ) -> Result<Vec<StewardProposal>, StewardError> {
        let created_at = input.created_at;
        let request = LocalModelRequest::from_input(&input);
        let response = self.backend.infer(request)?;
        decode_response(&response, self.identity.clone(), created_at)
    }
}

/// Input for a feature-gated local model Steward run.
#[derive(Clone, Debug)]
pub struct LocalModelStewardInput {
    created_at: DateTime<Utc>,
    task: String,
    evidence: Vec<LocalModelEvidence>,
}

impl LocalModelStewardInput {
    /// Creates input for a local model Steward run.
    pub fn new(created_at: DateTime<Utc>, task: impl Into<String>) -> Self {
        Self {
            created_at,
            task: task.into(),
            evidence: Vec::new(),
        }
    }

    /// Adds an evidence snippet to the model request.
    pub fn with_evidence(mut self, locator: impl Into<String>, text: impl Into<String>) -> Self {
        self.evidence.push(LocalModelEvidence {
            locator: locator.into(),
            text: text.into(),
        });
        self
    }
}

#[derive(Debug, Deserialize)]
struct ModelResponse {
    proposals: Vec<ModelProposal>,
}

#[derive(Debug, Deserialize)]
struct ModelProposal {
    action: ModelAction,
    rationale: String,
    citations: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
enum ModelAction {
    CreateCellDraft {
        anchors: Vec<SemanticAnchor>,
        payload_text: String,
    },
    LinkRevision {
        source: StateCellId,
        kind: ModelRevisionLinkKind,
        target: StateCellId,
    },
    AdjustConfidence {
        cell_id: StateCellId,
        proposed_confidence: f32,
    },
    LabelAnswerability {
        cell_id: StateCellId,
        questions: Vec<String>,
    },
    MarkFrontier {
        cell_id: StateCellId,
    },
    RequestVerification {
        cell_id: Option<StateCellId>,
        request: String,
    },
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ModelRevisionLinkKind {
    Predecessor,
    Supersedes,
    ConflictsWith,
    DerivesFrom,
}

impl From<ModelRevisionLinkKind> for RevisionLinkKind {
    fn from(value: ModelRevisionLinkKind) -> Self {
        match value {
            ModelRevisionLinkKind::Predecessor => Self::Predecessor,
            ModelRevisionLinkKind::Supersedes => Self::Supersedes,
            ModelRevisionLinkKind::ConflictsWith => Self::ConflictsWith,
            ModelRevisionLinkKind::DerivesFrom => Self::DerivesFrom,
        }
    }
}

fn decode_response(
    response: &str,
    steward: StewardIdentity,
    created_at: DateTime<Utc>,
) -> Result<Vec<StewardProposal>, StewardError> {
    let response: ModelResponse =
        serde_json::from_str(response).map_err(|_error| StewardError::InvalidModelResponse)?;

    response
        .proposals
        .into_iter()
        .map(|proposal| {
            StewardProposal::new(
                ProposalId::new(),
                steward.clone(),
                proposal.action.into_steward_action(),
                proposal.rationale,
                proposal.citations,
                created_at,
            )
        })
        .collect()
}

impl ModelAction {
    fn into_steward_action(self) -> StewardAction {
        match self {
            Self::CreateCellDraft {
                anchors,
                payload_text,
            } => StewardAction::CreateCellDraft {
                anchors,
                payload_text,
            },
            Self::LinkRevision {
                source,
                kind,
                target,
            } => StewardAction::LinkRevision {
                source,
                kind: kind.into(),
                target,
            },
            Self::AdjustConfidence {
                cell_id,
                proposed_confidence,
            } => StewardAction::AdjustConfidence {
                cell_id,
                proposed_confidence,
            },
            Self::LabelAnswerability { cell_id, questions } => {
                StewardAction::LabelAnswerability { cell_id, questions }
            }
            Self::MarkFrontier { cell_id } => StewardAction::MarkFrontier { cell_id },
            Self::RequestVerification { cell_id, request } => {
                StewardAction::RequestVerification { cell_id, request }
            }
        }
    }
}

fn build_prompt(task: &str, evidence: &[LocalModelEvidence]) -> String {
    let mut prompt = String::from(
        "You are the ContinuityDB database Steward. Emit only JSON with a proposals array. \
         Do not mutate truth directly. Preserve citation locators exactly.\n\n",
    );
    prompt.push_str("Task:\n");
    prompt.push_str(task);
    prompt.push_str("\n\nEvidence:\n");

    for item in evidence {
        prompt.push_str("- ");
        prompt.push_str(&item.locator);
        prompt.push_str(": ");
        prompt.push_str(&item.text);
        prompt.push('\n');
    }

    prompt
}
