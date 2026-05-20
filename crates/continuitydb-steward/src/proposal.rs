//! Steward proposal types.

use chrono::{DateTime, Utc};
use continuitydb_core::{SemanticAnchor, StateCellId};
use continuitydb_revision::RevisionLinkKind;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::StewardError;

/// Immutable identifier for a Steward proposal.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct ProposalId(Uuid);

impl ProposalId {
    /// Creates a random proposal identifier.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for ProposalId {
    fn default() -> Self {
        Self::new()
    }
}

/// Identifies the steward implementation or model that emitted a proposal.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StewardIdentity {
    name: String,
    version: String,
    prompt_profile: String,
}

impl StewardIdentity {
    /// Creates a validated Steward identity.
    pub fn new(
        name: impl Into<String>,
        version: impl Into<String>,
        prompt_profile: impl Into<String>,
    ) -> Result<Self, StewardError> {
        let name = name.into().trim().to_string();
        let version = version.into().trim().to_string();
        let prompt_profile = prompt_profile.into().trim().to_string();

        if name.is_empty() || version.is_empty() || prompt_profile.is_empty() {
            return Err(StewardError::EmptyStewardIdentity);
        }

        Ok(Self {
            name,
            version,
            prompt_profile,
        })
    }
}

/// Intent proposed by a database Steward.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum StewardAction {
    /// Proposes a new StateCell draft without committing it.
    CreateCellDraft {
        /// Semantic anchors for the proposed cell.
        anchors: Vec<SemanticAnchor>,
        /// Draft text payload.
        payload_text: String,
    },
    /// Proposes a revision link between two StateCell versions.
    LinkRevision {
        /// Source StateCell version.
        source: StateCellId,
        /// Revision link kind.
        kind: RevisionLinkKind,
        /// Target StateCell version.
        target: StateCellId,
    },
    /// Proposes a confidence value for an existing StateCell.
    AdjustConfidence {
        /// Target StateCell version.
        cell_id: StateCellId,
        /// Proposed confidence in the inclusive range 0.0..=1.0.
        proposed_confidence: f32,
    },
    /// Proposes answerability questions for an existing StateCell.
    LabelAnswerability {
        /// Target StateCell version.
        cell_id: StateCellId,
        /// Proposed questions.
        questions: Vec<String>,
    },
    /// Proposes that a StateCell should enter frontier monitoring.
    MarkFrontier {
        /// Target StateCell version.
        cell_id: StateCellId,
    },
    /// Proposes verification work.
    RequestVerification {
        /// Target StateCell version when the request is cell-specific.
        cell_id: Option<StateCellId>,
        /// Verification request description.
        request: String,
    },
}

/// Structured proposal emitted by a deterministic or model-backed Steward.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StewardProposal {
    id: ProposalId,
    steward: StewardIdentity,
    action: StewardAction,
    rationale: String,
    citations: Vec<String>,
    created_at: DateTime<Utc>,
}

impl StewardProposal {
    /// Creates a validated Steward proposal.
    pub fn new(
        id: ProposalId,
        steward: StewardIdentity,
        action: StewardAction,
        rationale: impl Into<String>,
        citations: Vec<String>,
        created_at: DateTime<Utc>,
    ) -> Result<Self, StewardError> {
        let rationale = rationale.into().trim().to_string();
        let citations: Vec<String> = citations
            .into_iter()
            .map(|citation| citation.trim().to_string())
            .filter(|citation| !citation.is_empty())
            .collect();

        if rationale.is_empty() {
            return Err(StewardError::EmptyRationale);
        }

        if citations.is_empty() {
            return Err(StewardError::MissingCitations);
        }

        Ok(Self {
            id,
            steward,
            action,
            rationale,
            citations,
            created_at,
        })
    }

    /// Returns this proposal's identifier.
    pub fn id(&self) -> ProposalId {
        self.id
    }

    /// Returns the proposing Steward identity.
    pub fn steward(&self) -> &StewardIdentity {
        &self.steward
    }

    /// Returns the proposed action.
    pub fn action(&self) -> &StewardAction {
        &self.action
    }

    /// Returns the proposal rationale.
    pub fn rationale(&self) -> &str {
        &self.rationale
    }

    /// Returns supporting citation locators.
    pub fn citations(&self) -> &[String] {
        &self.citations
    }

    /// Returns creation time.
    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }
}
