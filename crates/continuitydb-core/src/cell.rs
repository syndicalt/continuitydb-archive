//! StateCell domain model.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{CoreError, Evidence, ValidTimeRange};

/// Immutable identifier for a StateCell version.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct StateCellId(Uuid);

impl StateCellId {
    /// Creates a random StateCell identifier.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for StateCellId {
    fn default() -> Self {
        Self::new()
    }
}

/// Semantic anchor used to address a StateCell by meaning.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct SemanticAnchor(String);

impl SemanticAnchor {
    /// Creates a semantic anchor.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the inner anchor string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Visibility or applicability scope for a StateCell.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Scope {
    /// Personal scope.
    Personal(String),
    /// Project scope.
    Project(String),
    /// Team scope.
    Team(String),
    /// Organization scope.
    Organization(String),
    /// Global scope.
    Global,
    /// Task-specific scope.
    Task(String),
}

/// Lifecycle activation state.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ActivationState {
    /// Stored but not actively considered.
    Dormant,
    /// Available for normal checkout.
    Active,
    /// High-impact or uncertain enough to monitor.
    Frontier,
    /// Superseded or intentionally withdrawn from checkout.
    Retired,
}

/// Questions or intents a StateCell can help answer.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Answerability {
    questions: Vec<String>,
}

impl Answerability {
    /// Creates answerability from non-empty questions.
    pub fn new(questions: Vec<String>) -> Result<Self, CoreError> {
        let questions: Vec<String> = questions
            .into_iter()
            .map(|question| question.trim().to_string())
            .filter(|question| !question.is_empty())
            .collect();

        if questions.is_empty() {
            return Err(CoreError::EmptyAnswerability);
        }

        Ok(Self { questions })
    }

    /// Returns the normalized questions this StateCell can help answer.
    pub fn questions(&self) -> &[String] {
        &self.questions
    }
}

/// Estimated materialization cost for a StateCell.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CellCost {
    /// Estimated token cost for LLM inclusion.
    pub token_count: i64,
    /// Estimated compute cost units for materialization.
    pub compute_units: i64,
}

impl CellCost {
    /// Creates non-negative cost estimates.
    pub fn new(token_count: i64, compute_units: i64) -> Result<Self, CoreError> {
        if token_count < 0 || compute_units < 0 {
            return Err(CoreError::InvalidCost);
        }

        Ok(Self {
            token_count,
            compute_units,
        })
    }
}

/// Hybrid content payload for a StateCell.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum CellPayload {
    /// Text payload.
    Text(String),
    /// Structured JSON payload.
    Json(serde_json::Value),
    /// External binary/blob reference.
    BlobRef(String),
}

/// Append-only, evidence-backed unit of operational truth.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StateCell {
    /// Immutable cell version ID.
    pub id: StateCellId,
    /// Semantic anchors for lookup by meaning.
    pub anchors: Vec<SemanticAnchor>,
    /// Real-world validity interval.
    pub valid_time: ValidTimeRange,
    /// Scope for visibility and applicability.
    pub scope: Scope,
    /// Questions this cell can help answer.
    pub answerability: Answerability,
    /// Activation state for checkout/frontier selection.
    pub activation: ActivationState,
    /// Evidence supporting this cell version.
    pub evidence: Vec<Evidence>,
    /// Content payload.
    pub payload: CellPayload,
    /// Estimated cost.
    pub cost: CellCost,
}

impl StateCell {
    /// Creates a validated StateCell.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: StateCellId,
        anchors: Vec<SemanticAnchor>,
        valid_time: ValidTimeRange,
        scope: Scope,
        answerability: Answerability,
        evidence: Vec<Evidence>,
        payload: CellPayload,
        cost: CellCost,
    ) -> Result<Self, CoreError> {
        if anchors.is_empty() {
            return Err(CoreError::MissingSemanticAnchor);
        }

        if evidence.is_empty() {
            return Err(CoreError::MissingEvidence);
        }

        Ok(Self {
            id,
            anchors,
            valid_time,
            scope,
            answerability,
            activation: ActivationState::Active,
            evidence,
            payload,
            cost,
        })
    }
}
