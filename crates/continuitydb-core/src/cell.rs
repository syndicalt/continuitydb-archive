//! StateCell domain model.

use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};
use uuid::Uuid;

use crate::{Confidence, CoreError, Evidence, SystemTimeRange, ValidTimeRange};

/// Immutable identifier for a StateCell version.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct StateCellId(Uuid);

impl StateCellId {
    /// Creates a random StateCell identifier.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Creates a deterministic StateCell identifier from a 128-bit value.
    pub fn from_u128(value: u128) -> Self {
        Self(Uuid::from_u128(value))
    }
}

impl Default for StateCellId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for StateCellId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for StateCellId {
    type Err = uuid::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(value).map(Self)
    }
}

/// Immutable identifier for a database commit boundary.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct CommitId(Uuid);

impl CommitId {
    /// Creates a random commit identifier.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Returns the nil commit identifier used for legacy or uncommitted cells.
    pub fn nil() -> Self {
        Self(Uuid::nil())
    }

    /// Returns true when this is the nil commit identifier.
    pub fn is_nil(self) -> bool {
        self.0.is_nil()
    }
}

impl Default for CommitId {
    fn default() -> Self {
        Self::nil()
    }
}

impl fmt::Display for CommitId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for CommitId {
    type Err = uuid::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(value).map(Self)
    }
}

/// Immutable record of a database commit boundary.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CommitManifest {
    /// Commit identifier shared by all cells written in this boundary.
    pub commit_id: CommitId,
    /// System transaction time assigned to the commit.
    pub committed_at: chrono::DateTime<chrono::Utc>,
    /// StateCell identifiers written by this commit in append order.
    pub cell_ids: Vec<StateCellId>,
}

impl CommitManifest {
    /// Creates a commit manifest.
    pub fn new(
        commit_id: CommitId,
        committed_at: chrono::DateTime<chrono::Utc>,
        cell_ids: Vec<StateCellId>,
    ) -> Self {
        Self {
            commit_id,
            committed_at,
            cell_ids,
        }
    }
}

/// Relationship between two StateCell versions.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum RevisionLinkKind {
    /// Current cell directly follows a previous version.
    Predecessor,
    /// Current cell supersedes the target.
    Supersedes,
    /// Current cell conflicts with the target.
    ConflictsWith,
    /// Current cell was derived from the target.
    DerivesFrom,
}

/// Append-only record of a revision relationship between StateCell versions.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RevisionLinkRecord {
    /// Source StateCell version for the directed revision link.
    pub source: StateCellId,
    /// Revision relationship kind.
    pub kind: RevisionLinkKind,
    /// Target StateCell version for the directed revision link.
    pub target: StateCellId,
    /// System time when this revision link was recorded.
    pub recorded_at: chrono::DateTime<chrono::Utc>,
}

impl RevisionLinkRecord {
    /// Creates a revision link record.
    pub fn new(
        source: StateCellId,
        kind: RevisionLinkKind,
        target: StateCellId,
        recorded_at: chrono::DateTime<chrono::Utc>,
    ) -> Self {
        Self {
            source,
            kind,
            target,
            recorded_at,
        }
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
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
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
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
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

/// Learned or explicit utility signals for future context packing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct UtilityFeedback {
    /// How often this cell is relevant to requested work.
    pub relevance: Confidence,
    /// How useful the cell's freshness has been for decisions.
    pub recency: Confidence,
    /// How much this cell has affected successful decisions.
    pub decision_impact: Confidence,
}

impl UtilityFeedback {
    /// Creates utility feedback from bounded utility signals.
    pub fn new(relevance: Confidence, recency: Confidence, decision_impact: Confidence) -> Self {
        Self {
            relevance,
            recency,
            decision_impact,
        }
    }

    /// Returns a simple deterministic aggregate utility score.
    pub fn utility_score(self) -> f32 {
        (self.relevance.value() + self.recency.value() + self.decision_impact.value()) / 3.0
    }
}

/// Meaning of a StateCell dependency edge.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum CellDependencyKind {
    /// This cell depends on the target cell.
    DependsOn,
    /// This cell was caused by the target cell.
    CausedBy,
    /// This cell supports the target cell.
    Supports,
    /// This cell was derived from the target cell.
    DerivedFrom,
}

/// Dependency or causal reference from one StateCell to another.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CellDependency {
    /// Target StateCell identifier.
    pub target: StateCellId,
    /// Dependency edge meaning.
    pub kind: CellDependencyKind,
    /// Human-readable rationale for the dependency.
    pub rationale: String,
}

impl CellDependency {
    /// Creates a dependency reference.
    pub fn new(
        target: StateCellId,
        kind: CellDependencyKind,
        rationale: impl Into<String>,
    ) -> Self {
        Self {
            target,
            kind,
            rationale: rationale.into(),
        }
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
    /// Database transaction-time interval for this observed version.
    #[serde(default)]
    pub system_time: SystemTimeRange,
    /// Database commit boundary that wrote this version.
    #[serde(default)]
    pub commit_id: CommitId,
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
    /// Utility signals learned from prior use and outcomes.
    #[serde(default)]
    pub utility_feedback: UtilityFeedback,
    /// Dependency and causal references to other StateCells.
    #[serde(default)]
    pub dependencies: Vec<CellDependency>,
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
            system_time: SystemTimeRange::default(),
            commit_id: CommitId::nil(),
            scope,
            answerability,
            activation: ActivationState::Active,
            evidence,
            payload,
            cost,
            utility_feedback: UtilityFeedback::default(),
            dependencies: Vec::new(),
        })
    }
}
