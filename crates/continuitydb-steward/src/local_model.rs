//! Feature-gated local model Steward boundary.

use chrono::{DateTime, Utc};
use continuitydb_core::{SemanticAnchor, StateCellId};
use continuitydb_revision::RevisionLinkKind;
use serde::Deserialize;
use std::{
    ffi::OsStr,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use crate::{
    ProposalId, ProposalOutcome, ProposalPolicy, StewardAction, StewardError, StewardIdentity,
    StewardProposal,
};

/// Backend that runs local model inference for the database Steward.
pub trait LocalModelBackend {
    /// Runs inference for a prompt and returns a JSON proposal response.
    fn infer(&self, request: LocalModelRequest) -> Result<String, StewardError>;
}

/// Configuration for a local executable model runner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalExecutableRunnerConfig {
    executable: PathBuf,
    model_path: Option<PathBuf>,
    arguments: Vec<String>,
}

impl LocalExecutableRunnerConfig {
    /// Creates local executable runner configuration.
    pub fn new(executable: impl Into<PathBuf>) -> Self {
        Self {
            executable: executable.into(),
            model_path: None,
            arguments: Vec::new(),
        }
    }

    /// Sets the model path passed to the executable as `--model <path>`.
    pub fn with_model_path(mut self, model_path: impl Into<PathBuf>) -> Self {
        self.model_path = Some(model_path.into());
        self
    }

    /// Appends an argument passed to the executable after any model path.
    pub fn with_argument(mut self, argument: impl AsRef<OsStr>) -> Self {
        self.arguments
            .push(argument.as_ref().to_string_lossy().to_string());
        self
    }

    /// Returns the executable path.
    pub fn executable(&self) -> &Path {
        &self.executable
    }

    /// Returns deterministic command arguments.
    pub fn command_arguments(&self) -> Vec<String> {
        let mut arguments = Vec::new();
        if let Some(model_path) = &self.model_path {
            arguments.push("--model".to_string());
            arguments.push(model_path.to_string_lossy().to_string());
        }
        arguments.extend(self.arguments.clone());
        arguments
    }
}

/// Local executable backend for model inference.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalExecutableRunner {
    config: LocalExecutableRunnerConfig,
}

impl LocalExecutableRunner {
    /// Creates a runner from configuration.
    pub fn new(config: LocalExecutableRunnerConfig) -> Self {
        Self { config }
    }

    /// Returns runner configuration.
    pub fn config(&self) -> &LocalExecutableRunnerConfig {
        &self.config
    }
}

impl LocalModelBackend for LocalExecutableRunner {
    fn infer(&self, request: LocalModelRequest) -> Result<String, StewardError> {
        let mut child = Command::new(self.config.executable())
            .args(self.config.command_arguments())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|_error| StewardError::LocalModelExecutionFailed)?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(request.prompt().as_bytes())
                .map_err(|_error| StewardError::LocalModelExecutionFailed)?;
        }

        let output = child
            .wait_with_output()
            .map_err(|_error| StewardError::LocalModelExecutionFailed)?;

        if !output.status.success() {
            return Err(StewardError::LocalModelExecutionFailed);
        }

        String::from_utf8(output.stdout).map_err(|_error| StewardError::LocalModelExecutionFailed)
    }
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

/// Fixed proposal-quality case for local Steward model evaluation.
#[derive(Clone, Debug)]
pub struct StewardEvaluationCase {
    name: String,
    input: LocalModelStewardInput,
    expected_actions: Vec<StewardAction>,
    required_citations: Vec<String>,
    forbidden_rationale_terms: Vec<String>,
}

impl StewardEvaluationCase {
    /// Creates an evaluation case.
    pub fn new(
        name: impl Into<String>,
        created_at: DateTime<Utc>,
        task: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            input: LocalModelStewardInput::new(created_at, task),
            expected_actions: Vec::new(),
            required_citations: Vec::new(),
            forbidden_rationale_terms: Vec::new(),
        }
    }

    /// Adds an evidence snippet to this case's local model input.
    pub fn with_evidence(mut self, locator: impl Into<String>, text: impl Into<String>) -> Self {
        self.input = self.input.with_evidence(locator, text);
        self
    }

    /// Requires at least one emitted proposal to match the expected action.
    pub fn expect_action(mut self, action: StewardAction) -> Self {
        self.expected_actions.push(action);
        self
    }

    /// Requires emitted proposals to include this citation locator.
    pub fn require_citation(mut self, locator: impl Into<String>) -> Self {
        self.required_citations.push(locator.into());
        self
    }

    /// Rejects emitted proposal rationales containing this unsupported term.
    pub fn forbid_rationale_term(mut self, term: impl Into<String>) -> Self {
        self.forbidden_rationale_terms.push(term.into());
        self
    }
}

/// Deterministic suite for evaluating local Steward model proposal quality.
#[derive(Clone, Debug)]
pub struct StewardEvaluationSuite {
    cases: Vec<StewardEvaluationCase>,
}

impl StewardEvaluationSuite {
    /// Creates an evaluation suite from fixed cases.
    pub fn new(cases: Vec<StewardEvaluationCase>) -> Self {
        Self { cases }
    }

    /// Evaluates a local model Steward against all cases.
    pub fn evaluate<B>(&self, steward: &LocalModelSteward<B>) -> StewardEvaluationReport
    where
        B: LocalModelBackend,
    {
        let case_reports = self
            .cases
            .iter()
            .map(|case| evaluate_case(case, steward))
            .collect();

        StewardEvaluationReport { case_reports }
    }
}

/// Deterministic evaluation report for an entire suite.
#[derive(Clone, Debug, PartialEq)]
pub struct StewardEvaluationReport {
    case_reports: Vec<StewardEvaluationCaseReport>,
}

impl StewardEvaluationReport {
    /// Returns whether every case passed.
    pub fn passed(&self) -> bool {
        self.case_reports
            .iter()
            .all(StewardEvaluationCaseReport::passed)
    }

    /// Returns per-case evaluation reports.
    pub fn case_reports(&self) -> &[StewardEvaluationCaseReport] {
        &self.case_reports
    }
}

/// Deterministic evaluation report for one case.
#[derive(Clone, Debug, PartialEq)]
pub struct StewardEvaluationCaseReport {
    name: String,
    failures: Vec<StewardEvaluationFailure>,
}

impl StewardEvaluationCaseReport {
    /// Returns the case name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns whether this case passed.
    pub fn passed(&self) -> bool {
        self.failures.is_empty()
    }

    /// Returns deterministic failure reasons.
    pub fn failures(&self) -> &[StewardEvaluationFailure] {
        &self.failures
    }
}

/// Deterministic local model evaluation failure reason.
#[derive(Clone, Debug, PartialEq)]
pub enum StewardEvaluationFailure {
    /// The local model backend or decoder failed.
    ModelError,
    /// No emitted proposal matched an expected action.
    MissingExpectedAction {
        /// Expected action that was not emitted.
        action: StewardAction,
    },
    /// Required evidence citation was not preserved.
    MissingCitation {
        /// Missing citation locator.
        locator: String,
    },
    /// A proposal was rejected by deterministic policy.
    PolicyRejected {
        /// Rejection reasons emitted by policy.
        reasons: Vec<String>,
    },
    /// A proposal rationale included an unsupported claim marker.
    UnsupportedRationaleTerm {
        /// Forbidden term found in rationale text.
        term: String,
    },
}

/// Small embeddable open-source model candidate metadata.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SmallModelCandidate {
    model_id: &'static str,
    role: &'static str,
}

impl SmallModelCandidate {
    /// Returns the model identifier.
    pub fn model_id(&self) -> &'static str {
        self.model_id
    }

    /// Returns the candidate evaluation role.
    pub fn role(&self) -> &'static str {
        self.role
    }
}

/// Returns the fixed small model candidates for Steward evaluation.
pub fn small_model_candidates() -> &'static [SmallModelCandidate] {
    &[
        SmallModelCandidate {
            model_id: "Qwen/Qwen2.5-0.5B-Instruct",
            role: "default-feasibility",
        },
        SmallModelCandidate {
            model_id: "Qwen/Qwen3-0.6B",
            role: "current-reasoning",
        },
        SmallModelCandidate {
            model_id: "HuggingFaceTB/SmolLM2-360M-Instruct",
            role: "ultra-small-experimental",
        },
        SmallModelCandidate {
            model_id: "HuggingFaceTB/SmolLM2-135M-Instruct",
            role: "smoke-test-only",
        },
    ]
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

fn evaluate_case<B>(
    case: &StewardEvaluationCase,
    steward: &LocalModelSteward<B>,
) -> StewardEvaluationCaseReport
where
    B: LocalModelBackend,
{
    let mut failures = Vec::new();
    let proposals = match steward.propose(case.input.clone()) {
        Ok(proposals) => proposals,
        Err(_error) => {
            return StewardEvaluationCaseReport {
                name: case.name.clone(),
                failures: vec![StewardEvaluationFailure::ModelError],
            };
        }
    };

    for expected in &case.expected_actions {
        if proposals
            .iter()
            .all(|proposal| proposal.action() != expected)
        {
            failures.push(StewardEvaluationFailure::MissingExpectedAction {
                action: expected.clone(),
            });
        }
    }

    for required in &case.required_citations {
        if proposals
            .iter()
            .flat_map(StewardProposal::citations)
            .all(|citation| citation != required)
        {
            failures.push(StewardEvaluationFailure::MissingCitation {
                locator: required.clone(),
            });
        }
    }

    let policy = ProposalPolicy::strict();
    for proposal in &proposals {
        let decision = policy.evaluate(proposal, proposal.created_at());
        if decision.outcome() == ProposalOutcome::Rejected {
            failures.push(StewardEvaluationFailure::PolicyRejected {
                reasons: decision.reasons().to_vec(),
            });
        }
    }

    let rationales: Vec<String> = proposals
        .iter()
        .map(|proposal| proposal.rationale().to_ascii_lowercase())
        .collect();
    for term in &case.forbidden_rationale_terms {
        let normalized = term.to_ascii_lowercase();
        if rationales
            .iter()
            .any(|rationale| rationale.contains(&normalized))
        {
            failures
                .push(StewardEvaluationFailure::UnsupportedRationaleTerm { term: term.clone() });
        }
    }

    StewardEvaluationCaseReport {
        name: case.name.clone(),
        failures,
    }
}
