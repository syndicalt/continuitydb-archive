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

/// Native lifecycle attention signal for context selection.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AttentionSignal {
    /// How novel this cell is relative to known context.
    pub novelty: f32,
    /// How quickly this cell should influence near-term work.
    pub urgency: f32,
    /// How much decisions may change if this cell is ignored.
    pub impact: f32,
    /// How strongly this cell should resist lifecycle decay.
    pub decay_resistance: f32,
}

impl AttentionSignal {
    /// Creates a bounded attention signal.
    pub fn new(
        novelty: f32,
        urgency: f32,
        impact: f32,
        decay_resistance: f32,
    ) -> Result<Self, CoreError> {
        validate_attention_component(novelty)?;
        validate_attention_component(urgency)?;
        validate_attention_component(impact)?;
        validate_attention_component(decay_resistance)?;

        Ok(Self {
            novelty,
            urgency,
            impact,
            decay_resistance,
        })
    }

    /// Returns a deterministic aggregate attention score.
    pub fn salience_score(self) -> f32 {
        (self.novelty + self.urgency + self.impact + self.decay_resistance) / 4.0
    }

    /// Returns true when this signal should be surfaced to context consumers.
    pub fn is_recorded(self) -> bool {
        self.salience_score() > 0.0
    }
}

/// Native value-of-context signal for task-specific checkout decisions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ContextAffordance {
    /// Expected downstream task value if this cell is placed in context.
    pub expected_task_value: f32,
    /// Expected uncertainty reduction or information gain from including this cell.
    pub expected_information_gain: f32,
    /// Risk that the cell will be misused if its constraints are not visible.
    pub risk_of_misuse: f32,
    /// Degree to which the cell is ambiguous and needs context-preserving treatment.
    pub ambiguity: f32,
    /// Applicability to the current or recurring task family.
    pub applicability: f32,
    /// Relative pressure this cell places on scarce context resources.
    pub resource_pressure: f32,
}

impl ContextAffordance {
    /// Creates a bounded context affordance signal.
    pub fn new(
        expected_task_value: f32,
        expected_information_gain: f32,
        risk_of_misuse: f32,
        ambiguity: f32,
        applicability: f32,
        resource_pressure: f32,
    ) -> Result<Self, CoreError> {
        validate_context_affordance_component(expected_task_value)?;
        validate_context_affordance_component(expected_information_gain)?;
        validate_context_affordance_component(risk_of_misuse)?;
        validate_context_affordance_component(ambiguity)?;
        validate_context_affordance_component(applicability)?;
        validate_context_affordance_component(resource_pressure)?;

        Ok(Self {
            expected_task_value,
            expected_information_gain,
            risk_of_misuse,
            ambiguity,
            applicability,
            resource_pressure,
        })
    }

    /// Returns a deterministic value-of-context score for checkout ranking.
    pub fn context_affordance_score(self) -> f32 {
        let inclusion_pressure = self.expected_task_value
            + self.expected_information_gain
            + self.risk_of_misuse
            + self.ambiguity
            + self.applicability;
        ((inclusion_pressure / 5.0) * (1.0 - self.resource_pressure)).clamp(0.0, 1.0)
    }

    /// Returns true when this affordance should be surfaced to context consumers.
    pub fn is_recorded(self) -> bool {
        self.context_affordance_score() > 0.0
    }
}

/// Kind of missing context a StateCell can ask future checkout to scavenge.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum ContextGapKind {
    /// Supporting evidence is missing or insufficient.
    MissingEvidence,
    /// A decision, owner, or explicit resolution is missing.
    MissingDecision,
    /// A constraint, boundary, or applicability condition is missing.
    MissingConstraint,
    /// A dependency, causal predecessor, or related StateCell is missing.
    MissingDependency,
}

/// Structured missing-context contract carried by a StateCell.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContextGap {
    /// Type of missing context.
    pub kind: ContextGapKind,
    /// Question a context provider should answer before collapsing uncertainty.
    pub question: String,
    /// Why this gap matters for safe context use.
    pub rationale: String,
    /// Priority for surfacing or scavenging this gap.
    pub priority: Confidence,
}

impl ContextGap {
    /// Creates a validated missing-context contract.
    pub fn new(
        kind: ContextGapKind,
        question: impl Into<String>,
        rationale: impl Into<String>,
        priority: f32,
    ) -> Result<Self, CoreError> {
        let question = question.into().trim().to_string();
        if question.is_empty() {
            return Err(CoreError::EmptyContextGapQuestion);
        }

        let rationale = rationale.into().trim().to_string();
        if rationale.is_empty() {
            return Err(CoreError::EmptyContextGapRationale);
        }

        validate_context_gap_priority(priority)?;

        Ok(Self {
            kind,
            question,
            rationale,
            priority: Confidence::new(priority)?,
        })
    }
}

/// Kind of condition that can invalidate a StateCell belief.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum InvalidationConditionKind {
    /// New evidence contradicts this StateCell.
    ContradictoryEvidence,
    /// A required boundary, policy, or invariant no longer holds.
    BoundaryViolation,
    /// A time-bounded belief has expired.
    TemporalExpiry,
    /// A dependency or source belief has been invalidated.
    DependencyInvalidated,
}

/// Structured falsification contract carried by a StateCell.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InvalidationCondition {
    /// Type of invalidation condition.
    pub kind: InvalidationConditionKind,
    /// Concrete condition that should trigger revision or retirement.
    pub condition: String,
    /// Why this condition matters for safe context use.
    pub rationale: String,
    /// Priority for surfacing this invalidation condition.
    pub priority: Confidence,
}

impl InvalidationCondition {
    /// Creates a validated invalidation condition.
    pub fn new(
        kind: InvalidationConditionKind,
        condition: impl Into<String>,
        rationale: impl Into<String>,
        priority: f32,
    ) -> Result<Self, CoreError> {
        let condition = condition.into().trim().to_string();
        if condition.is_empty() {
            return Err(CoreError::EmptyInvalidationCondition);
        }

        let rationale = rationale.into().trim().to_string();
        if rationale.is_empty() {
            return Err(CoreError::EmptyInvalidationRationale);
        }

        validate_invalidation_priority(priority)?;

        Ok(Self {
            kind,
            condition,
            rationale,
            priority: Confidence::new(priority)?,
        })
    }
}

/// Distilled experience from a prior agent rollout or trajectory.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrajectoryMemory {
    /// Hypothesis, plan, or approach the agent attempted.
    pub hypothesis_tried: String,
    /// Useful progress made before the trajectory ended or failed.
    pub progress_made: String,
    /// Failure mode or stopping condition observed in the trajectory.
    pub failure_mode: String,
    /// Retained trace, artifact, or rollout locator supporting this memory.
    pub trace_locator: String,
    /// Confidence that this distilled lesson applies as recorded.
    pub confidence: Confidence,
    /// Reusable lesson for future checkout packets.
    pub reusable_lesson: String,
    /// Conditions under which the lesson should be reused.
    pub applicability_conditions: Vec<String>,
    /// Conditions that invalidate or retire this trajectory lesson.
    pub invalidation_conditions: Vec<String>,
    /// Preferred checkout strategy for reusing this experience.
    pub checkout_strategy: ContextPacketStrategy,
}

impl TrajectoryMemory {
    /// Creates validated rollout experience memory for future context reuse.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        hypothesis_tried: impl Into<String>,
        progress_made: impl Into<String>,
        failure_mode: impl Into<String>,
        trace_locator: impl Into<String>,
        confidence: f32,
        reusable_lesson: impl Into<String>,
        applicability_conditions: Vec<String>,
        invalidation_conditions: Vec<String>,
        checkout_strategy: ContextPacketStrategy,
    ) -> Result<Self, CoreError> {
        Ok(Self {
            hypothesis_tried: required_string(
                hypothesis_tried,
                CoreError::EmptyTrajectoryHypothesis,
            )?,
            progress_made: required_string(progress_made, CoreError::EmptyTrajectoryProgress)?,
            failure_mode: required_string(failure_mode, CoreError::EmptyTrajectoryFailureMode)?,
            trace_locator: required_string(trace_locator, CoreError::EmptyTrajectoryTraceLocator)?,
            confidence: Confidence::new(confidence)?,
            reusable_lesson: required_string(
                reusable_lesson,
                CoreError::EmptyTrajectoryReusableLesson,
            )?,
            applicability_conditions: required_string_vec(
                applicability_conditions,
                CoreError::EmptyTrajectoryApplicabilityCondition,
            )?,
            invalidation_conditions: required_string_vec(
                invalidation_conditions,
                CoreError::EmptyTrajectoryInvalidationCondition,
            )?,
            checkout_strategy,
        })
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

/// Lifecycle stage for a StateCell v2 memory unit.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum LifecycleStage {
    /// Raw observation has been captured but not yet promoted.
    #[default]
    Observed,
    /// Observation is currently believed by an agent or workflow.
    Believed,
    /// Observation conflicts with another belief or evidence source.
    Contradicted,
    /// Observation has been superseded by a newer StateCell.
    Superseded,
    /// Observation has been consolidated into a more stable memory.
    Consolidated,
    /// Observation has been abstracted into a reusable concept.
    Abstracted,
    /// Observation has become an executable instruction or policy.
    Operationalized,
    /// Observation has decayed below normal checkout utility.
    Decayed,
}

/// How a StateCell should be retained across context lifecycle turns.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum RetentionPolicy {
    /// Preserve until explicitly superseded or retired.
    #[default]
    Persistent,
    /// Preserve only while attention, evidence, or reinforcement keeps it useful.
    DecayUnlessReinforced,
    /// Use for the immediate task horizon only.
    Ephemeral,
}

/// How a StateCell may be used when compiling or consuming context.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum UsePolicy {
    /// The StateCell can be used directly when selected.
    #[default]
    UseDirectly,
    /// The StateCell can be used, but uncertainty should be preserved.
    HedgeBeforeUse,
    /// The StateCell should be verified before material decisions.
    VerifyBeforeUse,
    /// The StateCell should not be used as answer support.
    DoNotUseForAnswer,
}

/// How a StateCell can advance to a more durable lifecycle stage.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum PromotionPolicy {
    /// Promotion is controlled by an explicit revision or steward action.
    #[default]
    Manual,
    /// Promotion is allowed when supporting evidence reaches this count.
    EvidenceCount(usize),
    /// Promotion is allowed when maximum supporting confidence reaches this value.
    ConfidenceThreshold(Confidence),
    /// Promotion is allowed after repeated observed reinforcement.
    RepeatedObservation(usize),
}

/// Native policy contract for carrying StateCell context through its lifecycle.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ContextLifecyclePolicy {
    /// Retention behavior across future context turns.
    pub retention: RetentionPolicy,
    /// Consumption behavior when this cell is selected.
    pub use_policy: UsePolicy,
    /// Promotion behavior for lifecycle stabilization.
    pub promotion: PromotionPolicy,
}

/// Deterministic reason produced by lifecycle-policy evaluation.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum ContextLifecycleReason {
    /// The policy keeps the StateCell persistently available.
    PersistentRetention,
    /// The policy expects reinforcement or decay.
    DecayUnlessReinforced,
    /// The policy is scoped to the immediate task horizon.
    EphemeralRetention,
    /// The cell can be consumed directly.
    UseDirectly,
    /// The cell should preserve hedging when consumed.
    HedgeBeforeUse,
    /// The cell should be verified before material use.
    VerifyBeforeUse,
    /// The cell must not support an answer.
    DoNotUseForAnswer,
    /// Promotion requires an explicit external action.
    ManualPromotion,
    /// Evidence count reached the configured promotion threshold.
    PromotionEvidenceCountMet,
    /// Evidence count has not reached the configured promotion threshold.
    PromotionEvidenceCountMissing,
    /// Confidence reached the configured promotion threshold.
    PromotionThresholdMet,
    /// Confidence has not reached the configured promotion threshold.
    PromotionThresholdMissing,
    /// Repeated-observation promotion has not yet been materialized.
    RepeatedObservationRequired,
}

/// Deterministic lifecycle-policy evaluation for a StateCell at checkout time.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContextLifecycleEvaluation {
    /// Evaluated retention behavior.
    pub retention: RetentionPolicy,
    /// Evaluated use behavior.
    pub use_policy: UsePolicy,
    /// Evaluated promotion behavior.
    pub promotion: PromotionPolicy,
    /// Machine-readable policy reasons.
    pub reasons: Vec<ContextLifecycleReason>,
}

impl Default for ContextLifecycleEvaluation {
    fn default() -> Self {
        Self {
            retention: RetentionPolicy::Persistent,
            use_policy: UsePolicy::UseDirectly,
            promotion: PromotionPolicy::Manual,
            reasons: vec![
                ContextLifecycleReason::PersistentRetention,
                ContextLifecycleReason::UseDirectly,
                ContextLifecycleReason::ManualPromotion,
            ],
        }
    }
}

/// Cognitive or operational projection carried by a StateCell v2 memory unit.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum MemoryProjectionKind {
    /// What happened.
    Episodic,
    /// What is true or believed now.
    Semantic,
    /// What to do next time.
    Procedural,
    /// Instruction or policy that should guide future behavior.
    Policy,
    /// What remains unresolved or risky.
    Uncertainty,
}

/// Task-facing projection of a StateCell into a specific memory form.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MemoryProjection {
    /// Projection memory form.
    pub kind: MemoryProjectionKind,
    /// Model-facing projection text.
    pub text: String,
    /// Confidence assigned to this projection.
    pub confidence: Confidence,
    /// Estimated materialization cost for this projection.
    pub cost: CellCost,
}

impl MemoryProjection {
    /// Creates a validated memory projection.
    pub fn new(
        kind: MemoryProjectionKind,
        text: impl Into<String>,
        confidence: Confidence,
        cost: CellCost,
    ) -> Result<Self, CoreError> {
        let text = text.into().trim().to_string();
        if text.is_empty() {
            return Err(CoreError::EmptyProjection);
        }

        Ok(Self {
            kind,
            text,
            confidence,
            cost,
        })
    }
}

/// Context rendering profile for StateCell v2 projection packets.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum ContextProfile {
    /// Context for debugging a failure or contradiction.
    Debugging,
    /// Context for planning future work.
    Planning,
    /// Context for safe execution.
    Execution,
    /// Context for audit or review.
    Audit,
    /// Context for reflection and consolidation.
    Reflection,
}

/// Deterministic compiler strategy used to shape model-facing context.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum ContextPacketStrategy {
    /// Existing profile-driven packet rendering.
    #[default]
    RawProjection,
    /// Operational current-state brief for direct use.
    OperationalBrief,
    /// Revision-aware context explaining changed or superseded state.
    RevisionCapsule,
    /// Uncertainty, surprise, calibration, or verification caveat.
    UncertaintyBrief,
    /// Missing-context guidance for scavenging before answer collapse.
    ScavengingBrief,
    /// Falsification guidance for unsafe or invalidated context.
    FalsificationBrief,
}

/// Compilation path that produced a model-facing context packet.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum ContextCompilerPolicy {
    /// Default checkout compiler that turns continuity state into task-ready context.
    #[default]
    Automatic,
    /// Static StateCell rendering used for diagnostics and ablation comparisons.
    RawBaseline,
    /// Validated model-assisted compiler proposal.
    ModelAssisted,
}

/// Abstraction level chosen by the context compiler.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum ContextAbstractionLevel {
    /// Native StateCell packet with no compiler guidance.
    #[default]
    Raw,
    /// Concise current-state operational context.
    Brief,
    /// Compact narrative for revision, conflict, or supersession.
    Capsule,
    /// Citation-heavy packet for audit or low-trust situations.
    EvidenceDense,
    /// Missing-context packet for scavenging before answer collapse.
    Scavenging,
    /// Falsification-oriented packet for unsafe assumptions.
    Falsification,
}

/// Why a context packet is being planned for model-facing use.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum ContextPacketPurpose {
    /// Direct answer support from selected StateCell evidence and projections.
    AnswerSupport,
    /// Guidance for revising a prior belief, plan, or StateCell interpretation.
    RevisionGuidance,
    /// Guidance that prevents unsafe or unsupported action.
    SafetyGuidance,
    /// Reuse of a retained trajectory lesson.
    TrajectoryReuse,
    /// Guidance that preserves uncertainty, surprise, or calibration caveats.
    UncertaintyGuidance,
    /// Guidance for falsifying or retiring stale assumptions.
    Falsification,
    /// Evidence-dense context for review, audit, or traceability.
    Audit,
}

/// Source contract a planned context packet must preserve or may include.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum ContextPacketRequirement {
    /// Compact revision-neighborhood context.
    RevisionContext,
    /// Native lifecycle policy and lifecycle evaluation.
    LifecyclePolicy,
    /// Native uncertainty, surprise, expectation, or calibration metadata.
    Uncertainty,
    /// Native invalidation or falsification conditions.
    Invalidation,
    /// Conditions that make a retained trajectory lesson applicable.
    TrajectoryApplicability,
    /// Native value-of-context affordance signals.
    ContextAffordance,
    /// Citation locators that support the planned packet.
    EvidenceCitations,
}

/// Auditable plan describing the packet shape checkout should expose.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ContextPacketPlan {
    /// Primary purpose for this packet.
    pub purpose: ContextPacketPurpose,
    /// Contracts required for this packet to satisfy its purpose.
    pub required_contracts: Vec<ContextPacketRequirement>,
    /// Contracts useful to include when budget allows.
    pub optional_contracts: Vec<ContextPacketRequirement>,
    /// Planned abstraction level for the packet.
    pub abstraction_level: ContextAbstractionLevel,
    /// Planned compiler strategy for rendering the packet.
    pub strategy: ContextPacketStrategy,
    /// Auditable reason tags explaining this plan.
    pub reason_tags: Vec<String>,
    /// Evidence locators supporting this plan.
    pub evidence_locators: Vec<String>,
}

impl ContextPacketPlan {
    /// Creates a validated context packet plan.
    pub fn new(
        purpose: ContextPacketPurpose,
        required_contracts: Vec<ContextPacketRequirement>,
        optional_contracts: Vec<ContextPacketRequirement>,
        abstraction_level: ContextAbstractionLevel,
        strategy: ContextPacketStrategy,
        reason_tags: Vec<String>,
        evidence_locators: Vec<String>,
    ) -> Result<Self, CoreError> {
        let reason_tags =
            non_empty_strings(reason_tags, CoreError::EmptyContextCompilerProposalReason)?;
        let evidence_locators = non_empty_strings(
            evidence_locators,
            CoreError::EmptyContextCompilerProposalEvidence,
        )?;
        if !strategy.is_compatible_abstraction_level(abstraction_level) {
            return Err(CoreError::InvalidContextCompilerProposalShape);
        }

        Ok(Self {
            purpose,
            required_contracts: unique_packet_requirements(required_contracts),
            optional_contracts: unique_packet_requirements(optional_contracts),
            abstraction_level,
            strategy,
            reason_tags,
            evidence_locators,
        })
    }
}

/// Structured proposal for model-assisted context packet compilation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ContextCompilerProposal {
    /// StateCell version this proposal is allowed to shape.
    pub target_cell_id: StateCellId,
    /// Proposed compiler strategy for the target packet.
    pub strategy: ContextPacketStrategy,
    /// Proposed abstraction level for the target packet.
    pub abstraction_level: ContextAbstractionLevel,
    /// Auditable reason tags explaining why the shape was proposed.
    pub reason_tags: Vec<String>,
    /// Evidence locators that support the proposed shape.
    pub evidence_locators: Vec<String>,
    /// Optional model-assisted packet lines. Checkout only materializes lines
    /// whose citations are supported by the source StateCell.
    #[serde(default)]
    pub proposed_lines: Vec<ContextCompilerProposalLine>,
}

/// Citation-backed packet line proposed by a model-assisted context compiler.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ContextCompilerProposalLine {
    /// Proposed model-facing text.
    pub text: String,
    /// Source StateCell citation locators that support the proposed text.
    pub citations: Vec<String>,
    /// Estimated token cost for the proposed text.
    pub token_count: i64,
}

impl ContextCompilerProposal {
    /// Creates a validated model-assisted context compiler proposal.
    pub fn new(
        target_cell_id: StateCellId,
        strategy: ContextPacketStrategy,
        abstraction_level: ContextAbstractionLevel,
        reason_tags: Vec<String>,
        evidence_locators: Vec<String>,
    ) -> Result<Self, CoreError> {
        let reason_tags =
            non_empty_strings(reason_tags, CoreError::EmptyContextCompilerProposalReason)?;
        let evidence_locators = non_empty_strings(
            evidence_locators,
            CoreError::EmptyContextCompilerProposalEvidence,
        )?;
        if !strategy.is_compatible_abstraction_level(abstraction_level) {
            return Err(CoreError::InvalidContextCompilerProposalShape);
        }
        Ok(Self {
            target_cell_id,
            strategy,
            abstraction_level,
            reason_tags,
            evidence_locators,
            proposed_lines: Vec::new(),
        })
    }

    /// Attaches validated proposed packet lines to this compiler proposal.
    pub fn with_proposed_lines(mut self, proposed_lines: Vec<ContextCompilerProposalLine>) -> Self {
        if self.strategy == ContextPacketStrategy::RawProjection {
            self.proposed_lines.clear();
            return self;
        }

        let mut unique_proposed_lines = Vec::new();
        for proposed_line in proposed_lines {
            if !unique_proposed_lines
                .iter()
                .any(|existing| existing == &proposed_line)
            {
                unique_proposed_lines.push(proposed_line);
            }
        }
        self.proposed_lines = unique_proposed_lines;
        self
    }
}

impl ContextCompilerProposalLine {
    /// Creates a citation-backed proposed packet line.
    pub fn new(
        text: impl Into<String>,
        citations: Vec<String>,
        token_count: i64,
    ) -> Result<Self, CoreError> {
        let text = required_string(text, CoreError::EmptyContextCompilerProposalLine)?;
        let citations = required_unique_string_vec(
            citations,
            CoreError::EmptyContextCompilerProposalLineCitation,
        )?;
        if token_count <= 0 {
            return Err(CoreError::InvalidCost);
        }

        Ok(Self {
            text,
            citations,
            token_count,
        })
    }
}

impl ContextPacketStrategy {
    /// Returns the default abstraction level produced by this packet strategy.
    pub fn default_abstraction_level(self) -> ContextAbstractionLevel {
        match self {
            Self::RawProjection => ContextAbstractionLevel::Raw,
            Self::OperationalBrief => ContextAbstractionLevel::Brief,
            Self::RevisionCapsule => ContextAbstractionLevel::Capsule,
            Self::UncertaintyBrief => ContextAbstractionLevel::Brief,
            Self::ScavengingBrief => ContextAbstractionLevel::Scavenging,
            Self::FalsificationBrief => ContextAbstractionLevel::Falsification,
        }
    }

    /// Returns whether an explicit abstraction level is coherent with this strategy.
    pub fn is_compatible_abstraction_level(
        self,
        abstraction_level: ContextAbstractionLevel,
    ) -> bool {
        abstraction_level == self.default_abstraction_level()
            || (self != Self::RawProjection
                && abstraction_level == ContextAbstractionLevel::EvidenceDense)
    }
}

fn non_empty_strings(values: Vec<String>, error: CoreError) -> Result<Vec<String>, CoreError> {
    let mut non_empty = Vec::new();
    for value in values {
        if value.trim().is_empty() {
            continue;
        }
        if !non_empty.iter().any(|existing| existing == &value) {
            non_empty.push(value);
        }
    }
    if non_empty.is_empty() {
        return Err(error);
    }
    Ok(non_empty)
}

fn unique_packet_requirements(
    requirements: Vec<ContextPacketRequirement>,
) -> Vec<ContextPacketRequirement> {
    let mut unique = Vec::new();
    for requirement in requirements {
        if !unique.contains(&requirement) {
            unique.push(requirement);
        }
    }
    unique
}

fn required_string(value: impl Into<String>, error: CoreError) -> Result<String, CoreError> {
    let value = value.into().trim().to_string();
    if value.is_empty() {
        return Err(error);
    }
    Ok(value)
}

fn required_string_vec(values: Vec<String>, error: CoreError) -> Result<Vec<String>, CoreError> {
    if values.is_empty() {
        return Err(error);
    }

    values
        .into_iter()
        .map(|value| required_string(value, error.clone()))
        .collect()
}

fn required_unique_string_vec(
    values: Vec<String>,
    error: CoreError,
) -> Result<Vec<String>, CoreError> {
    let values = required_string_vec(values, error)?;
    let mut unique = Vec::new();
    for value in values {
        if !unique.iter().any(|existing| existing == &value) {
            unique.push(value);
        }
    }
    Ok(unique)
}

/// Model-facing packet compiled from StateCell v2 projections.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContextPacket {
    /// Rendering profile used to choose projection priority.
    pub profile: ContextProfile,
    /// Compiler strategy used to shape this model-facing packet.
    #[serde(default)]
    pub strategy: ContextPacketStrategy,
    /// Compiler policy used to choose this packet shape.
    #[serde(default)]
    pub compiler_policy: ContextCompilerPolicy,
    /// Abstraction level chosen by the compiler policy.
    #[serde(default)]
    pub abstraction_level: ContextAbstractionLevel,
    /// Auditable reason tags explaining the compiler's packet-shaping choice.
    #[serde(default)]
    pub compiler_reason_tags: Vec<String>,
    /// Evidence locators supporting the compiler's packet-shaping choice.
    #[serde(default)]
    pub compiler_evidence_locators: Vec<String>,
    /// Typed packet plan that explains purpose and required source contracts.
    #[serde(default)]
    pub plan: Option<ContextPacketPlan>,
    /// StateCell origin that produced this packet.
    #[serde(default)]
    pub origin: Option<ContextPacketOrigin>,
    /// Deterministic metadata explaining why this packet was selected.
    #[serde(default)]
    pub selection: Option<ContextPacketSelection>,
    /// Selected projection lines.
    pub lines: Vec<String>,
    /// Structured entries behind the selected text lines.
    #[serde(default)]
    pub entries: Vec<ContextPacketEntry>,
    /// Citation locators supporting this packet's source StateCell.
    #[serde(default)]
    pub citations: Vec<String>,
    /// Dependency and causality context preserved for packet-only consumers.
    #[serde(default)]
    pub dependency_context: Vec<ContextPacketDependencyContext>,
    /// Compact revision-neighborhood context preserved for packet-only consumers.
    #[serde(default)]
    pub revision_context: Vec<ContextPacketRevisionContext>,
    /// Total estimated token cost.
    pub token_count: i64,
}

/// StateCell origin metadata preserved for packet-only context consumers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContextPacketOrigin {
    /// StateCell version that produced this packet.
    pub cell_id: StateCellId,
    /// Semantic anchors associated with the source StateCell.
    pub anchors: Vec<SemanticAnchor>,
    /// Scope associated with the source StateCell.
    #[serde(default)]
    pub scope: Option<Scope>,
    /// Real-world validity interval associated with the source StateCell.
    #[serde(default)]
    pub valid_time: Option<ValidTimeRange>,
    /// System transaction-time interval associated with the source StateCell.
    #[serde(default)]
    pub system_time: Option<SystemTimeRange>,
    /// Lifecycle stage represented by this packet.
    pub lifecycle_stage: LifecycleStage,
    /// Activation state represented by this packet.
    pub activation: ActivationState,
    /// Commit boundary that wrote the source StateCell.
    pub commit_id: CommitId,
}

/// Deterministic selection metadata for agent-facing context packets.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContextPacketSelection {
    /// Maximum evidence confidence observed on the source StateCell.
    pub max_confidence: Confidence,
    /// Deterministic action stance recommended for consuming this packet.
    #[serde(default)]
    pub epistemic_action: EpistemicAction,
    /// Deterministic reasons that produced the recommended action stance.
    #[serde(default)]
    pub epistemic_action_reasons: Vec<EpistemicActionReason>,
    /// Derived pressure signals for revision, scavenging, and checkout priority.
    #[serde(default)]
    pub epistemic_pressure: EpistemicPressure,
    /// Learned utility score used by checkout ranking.
    pub utility_score: f32,
    /// Native uncertainty score carried by the source StateCell.
    pub uncertainty_score: Confidence,
    /// Native surprise value in bits carried by the source StateCell.
    pub surprise_bits: f32,
    /// Baseline expectation that produced the native surprise value, when known.
    #[serde(default)]
    pub expectation: Option<EpistemicExpectation>,
    /// Native empirical calibration signal carried by the source StateCell.
    #[serde(default)]
    pub calibration: EpistemicCalibration,
    /// Absolute calibration error between expected confidence and observed frequency.
    #[serde(default)]
    pub calibration_error: f32,
    /// Native lifecycle attention signal carried by the source StateCell.
    #[serde(default)]
    pub attention: AttentionSignal,
    /// Aggregate salience score derived from native attention signals.
    #[serde(default)]
    pub salience_score: f32,
    /// Native value-of-context signal carried by the source StateCell.
    #[serde(default)]
    pub context_affordance: ContextAffordance,
    /// Structured missing-context contracts carried by the source StateCell.
    #[serde(default)]
    pub context_gaps: Vec<ContextGap>,
    /// Structured invalidation conditions carried by the source StateCell.
    #[serde(default)]
    pub invalidation_conditions: Vec<InvalidationCondition>,
    /// Distilled rollout experience carried by the source StateCell.
    #[serde(default)]
    pub trajectory_memory: Option<TrajectoryMemory>,
    /// Aggregate value-of-context score derived from native affordance signals.
    #[serde(default)]
    pub context_affordance_score: f32,
    /// Native context lifecycle policy evaluation carried by the source StateCell.
    #[serde(default)]
    pub lifecycle_policy: ContextLifecycleEvaluation,
    /// Questions this packet's source StateCell is intended to answer.
    #[serde(default)]
    pub answerability_questions: Vec<String>,
    /// Deterministic reason tags explaining packet selection.
    pub reasons: Vec<ContextPacketSelectionReason>,
}

/// Deterministic reason a StateCell v2 context packet should be useful.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ContextPacketSelectionReason {
    /// The source StateCell carries supporting evidence confidence.
    EvidenceConfidence,
    /// Learned utility feedback contributes to future checkout ranking.
    UtilityFeedback,
    /// The source StateCell is in a lifecycle stage relevant to context.
    LifecycleStage,
    /// The requested context profile selected one or more projections.
    ProjectionProfile,
    /// Native uncertainty was surfaced into the packet.
    NativeUncertainty,
    /// Native empirical calibration was surfaced into the packet.
    EpistemicCalibration,
    /// Native attention/salience was surfaced into the packet.
    AttentionSignal,
    /// Native answerability intent was preserved in packet selection metadata.
    Answerability,
    /// Native lifecycle policy was surfaced into packet selection metadata.
    LifecyclePolicy,
    /// Native value-of-context affordance was surfaced into packet selection metadata.
    ContextAffordance,
    /// Native missing-context gap was surfaced into packet selection metadata.
    ContextGap,
    /// Native invalidation condition was surfaced into packet selection metadata.
    InvalidationCondition,
    /// Native rollout or trajectory experience was surfaced into packet selection metadata.
    TrajectoryMemory,
}

/// Deterministic stance a context consumer should take toward a StateCell packet.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum EpistemicAction {
    /// Use the packet as ordinary selected context.
    #[default]
    Use,
    /// Use cautiously and preserve uncertainty in downstream wording or action.
    Hedge,
    /// Verify the packet before relying on it for a material decision.
    Verify,
    /// Revise a prior belief or plan because observed evidence violated the baseline.
    Revise,
    /// Actively search for missing context or evidence before collapsing the answer.
    Scavenge,
}

/// Deterministic reason an epistemic action was selected.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum EpistemicActionReason {
    /// Native uncertainty crossed the high-uncertainty threshold.
    HighUncertainty,
    /// Native surprise crossed the high-surprise threshold.
    HighSurprise,
    /// Native uncertainty crossed the moderate-uncertainty threshold.
    ModerateUncertainty,
    /// Supporting evidence confidence is below the ordinary-use threshold.
    LowEvidenceConfidence,
    /// Empirical calibration shows confidence has been unreliable.
    MiscalibratedConfidence,
}

/// Deterministic epistemic pressure signals derived from StateCell v2 metadata.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EpistemicPressure {
    /// Pressure to revise a prior belief or plan.
    pub revision_pressure: f32,
    /// Pressure to scavenge for missing evidence before collapsing context.
    pub scavenging_pressure: f32,
    /// Aggregate pressure useful for checkout prioritization and summaries.
    pub checkout_pressure: f32,
}

/// Dependency or causal context for a StateCell v2 packet.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ContextPacketDependencyContext {
    /// Target StateCell identifier.
    pub target: StateCellId,
    /// Dependency edge meaning.
    pub kind: CellDependencyKind,
    /// Human-readable rationale for the dependency.
    pub rationale: String,
    /// Semantic anchors on the dependency target cell.
    #[serde(default)]
    pub anchors: Vec<SemanticAnchor>,
    /// Evidence citations supporting the dependency target cell.
    #[serde(default)]
    pub citations: Vec<String>,
}

/// Compact revision-neighborhood context for a StateCell v2 packet.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContextPacketRevisionContext {
    /// Related StateCell reached through the revision link.
    pub related_cell_id: StateCellId,
    /// Revision relationship from the packet origin's perspective.
    pub relation: ContextPacketRevisionRelation,
    /// Native revision link kind connecting the cells.
    pub kind: RevisionLinkKind,
    /// Semantic anchors on the related StateCell.
    #[serde(default)]
    pub anchors: Vec<SemanticAnchor>,
    /// Citation locators supporting the related StateCell.
    pub citations: Vec<String>,
    /// Maximum confidence across the related StateCell evidence.
    pub max_confidence: Confidence,
    /// Activation state of the related StateCell.
    pub activation: ActivationState,
}

/// Direction of a revision link relative to the packet origin.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ContextPacketRevisionRelation {
    /// The packet origin is the source of the revision link.
    SourceToTarget,
    /// The packet origin is the target of the revision link.
    TargetFromSource,
    /// The packet origin appears as both source and target.
    SelfLink,
}

/// Structured context packet line with source and confidence metadata.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContextPacketEntry {
    /// Source contract that produced this packet line.
    pub source: ContextPacketEntrySource,
    /// Model-facing text included in the packet.
    pub text: String,
    /// Confidence associated with this packet entry.
    pub confidence: Confidence,
    /// Estimated token cost charged for this entry.
    pub token_count: i64,
}

/// StateCell v2 source contract behind a context packet entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ContextPacketEntrySource {
    /// Native lifecycle policy rendered from `ContextLifecyclePolicy`.
    NativeLifecyclePolicy,
    /// Native uncertainty rationale rendered from `EpistemicUncertainty`.
    NativeUncertainty,
    /// Native expectation baseline rendered from `EpistemicExpectation`.
    NativeExpectation,
    /// Native empirical calibration rendered from `EpistemicCalibration`.
    NativeCalibration,
    /// Native lifecycle attention signal rendered from `AttentionSignal`.
    NativeAttention,
    /// Native value-of-context signal rendered from `ContextAffordance`.
    NativeContextAffordance,
    /// Native missing-context question rendered from `ContextGap`.
    NativeContextGap,
    /// Native invalidation condition rendered from `InvalidationCondition`.
    NativeInvalidationCondition,
    /// Native rollout or trajectory experience rendered from `TrajectoryMemory`.
    NativeTrajectoryMemory,
    /// Native revision-neighborhood context rendered during checkout.
    NativeRevisionContext,
    /// Native dependency or causal context rendered during checkout.
    NativeDependencyContext,
    /// Validated model-assisted compiler line rendered during checkout.
    ModelAssistedCompilerLine,
    /// Task-facing projection rendered from a specific memory form.
    Projection(MemoryProjectionKind),
}

/// Baseline expectation used to derive surprise for a StateCell v2 memory unit.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EpistemicExpectation {
    /// Human/model-facing label for the expected outcome.
    pub label: String,
    /// Prior probability assigned to the expected outcome before observation.
    pub prior_probability: Confidence,
    /// Probability assigned to the outcome that was actually observed.
    pub observed_probability: Confidence,
    /// Absolute probability movement between prior belief and observed outcome.
    #[serde(default)]
    pub probability_delta: f32,
    /// Surprisal of the observed outcome under the prior, in bits.
    pub surprise_bits: f32,
}

impl EpistemicExpectation {
    const MIN_OBSERVED_PROBABILITY_FOR_SURPRISE: f32 = 0.000_001;

    /// Creates a trace from an observed outcome probability.
    pub fn new(
        label: impl Into<String>,
        prior_probability: Confidence,
        observed_probability: Confidence,
    ) -> Result<Self, CoreError> {
        let label = label.into().trim().to_string();
        if label.is_empty() {
            return Err(CoreError::EmptyExpectationLabel);
        }

        let probability_delta = probability_delta(prior_probability, observed_probability);
        let surprise_bits = surprise_bits(observed_probability.value());

        Ok(Self {
            label,
            prior_probability,
            observed_probability,
            probability_delta,
            surprise_bits,
        })
    }

    /// Creates a trace for whether the expected binary outcome was observed.
    pub fn from_expected_outcome(
        label: impl Into<String>,
        prior_probability: Confidence,
        expected_observed: bool,
    ) -> Result<Self, CoreError> {
        let observed_probability = if expected_observed {
            prior_probability
        } else {
            Confidence::new(round_probability(1.0 - prior_probability.value()))?
        };

        Self::new(label, prior_probability, observed_probability)
    }
}

/// Native epistemic uncertainty carried by a StateCell v2 memory unit.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EpistemicUncertainty {
    /// Bounded uncertainty score where zero means no recorded uncertainty.
    pub score: Confidence,
    /// Surprisal produced by evidence relative to the prior baseline.
    pub surprise_bits: f32,
    /// Baseline expectation that produced the surprise value, when recorded.
    #[serde(default)]
    pub expectation: Option<EpistemicExpectation>,
    /// Human/model-facing rationale for why the cell remains uncertain.
    pub rationale: String,
}

impl Default for EpistemicUncertainty {
    fn default() -> Self {
        Self {
            score: Confidence::ZERO,
            surprise_bits: 0.0,
            expectation: None,
            rationale: String::new(),
        }
    }
}

impl EpistemicUncertainty {
    /// Creates validated native uncertainty metadata.
    pub fn new(
        score: Confidence,
        surprise_bits: f32,
        rationale: impl Into<String>,
    ) -> Result<Self, CoreError> {
        if !surprise_bits.is_finite() || surprise_bits < 0.0 {
            return Err(CoreError::InvalidSurpriseBits {
                value: surprise_bits,
            });
        }

        let rationale = rationale.into().trim().to_string();
        if rationale.is_empty() {
            return Err(CoreError::EmptyUncertaintyRationale);
        }

        Ok(Self {
            score,
            surprise_bits,
            expectation: None,
            rationale,
        })
    }

    /// Creates uncertainty metadata whose surprise is derived from an expectation trace.
    pub fn from_expectation(
        score: Confidence,
        expectation: EpistemicExpectation,
        rationale: impl Into<String>,
    ) -> Result<Self, CoreError> {
        let rationale = rationale.into().trim().to_string();
        if rationale.is_empty() {
            return Err(CoreError::EmptyUncertaintyRationale);
        }

        Ok(Self {
            score,
            surprise_bits: expectation.surprise_bits,
            expectation: Some(expectation),
            rationale,
        })
    }

    /// Returns true when uncertainty should be surfaced to checkout context.
    pub fn is_recorded(&self) -> bool {
        !self.rationale.is_empty()
    }
}

/// Empirical calibration trace for a belief source or model-facing claim family.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EpistemicCalibration {
    /// Confidence the source or model assigned before outcomes were checked.
    pub expected_confidence: Confidence,
    /// Observed frequency of the expected outcome across comparable checks.
    pub observed_frequency: Confidence,
    /// Number of observations behind this calibration trace.
    pub sample_count: usize,
    /// Absolute gap between expected confidence and observed frequency.
    #[serde(default)]
    pub calibration_error: f32,
    /// Human/model-facing rationale explaining the calibration signal.
    pub rationale: String,
}

impl Default for EpistemicCalibration {
    fn default() -> Self {
        Self {
            expected_confidence: Confidence::ZERO,
            observed_frequency: Confidence::ZERO,
            sample_count: 0,
            calibration_error: 0.0,
            rationale: String::new(),
        }
    }
}

impl EpistemicCalibration {
    /// Creates validated native calibration metadata.
    pub fn new(
        expected_confidence: Confidence,
        observed_frequency: Confidence,
        sample_count: usize,
        rationale: impl Into<String>,
    ) -> Result<Self, CoreError> {
        if sample_count == 0 {
            return Err(CoreError::InvalidCalibrationSampleCount);
        }

        let rationale = rationale.into().trim().to_string();
        if rationale.is_empty() {
            return Err(CoreError::EmptyCalibrationRationale);
        }

        Ok(Self {
            expected_confidence,
            observed_frequency,
            sample_count,
            calibration_error: probability_delta(expected_confidence, observed_frequency),
            rationale,
        })
    }

    /// Returns true when calibration should be surfaced to checkout context.
    pub fn is_recorded(&self) -> bool {
        !self.rationale.is_empty()
    }
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
    /// StateCell v2 lifecycle stage.
    #[serde(default)]
    pub lifecycle_stage: LifecycleStage,
    /// StateCell v2 native context lifecycle policy.
    #[serde(default)]
    pub lifecycle_policy: ContextLifecyclePolicy,
    /// StateCell v2 task-facing memory projections.
    #[serde(default)]
    pub projections: Vec<MemoryProjection>,
    /// StateCell v2 native epistemic uncertainty metadata.
    #[serde(default)]
    pub uncertainty: EpistemicUncertainty,
    /// StateCell v2 native empirical calibration metadata.
    #[serde(default)]
    pub calibration: EpistemicCalibration,
    /// StateCell v2 native lifecycle attention signal.
    #[serde(default)]
    pub attention: AttentionSignal,
    /// StateCell v2 native value-of-context signal.
    #[serde(default)]
    pub context_affordance: ContextAffordance,
    /// StateCell v2 native missing-context contracts.
    #[serde(default)]
    pub context_gaps: Vec<ContextGap>,
    /// StateCell v2 native invalidation conditions.
    #[serde(default)]
    pub invalidation_conditions: Vec<InvalidationCondition>,
    /// StateCell v2 native distilled rollout or trajectory memory.
    #[serde(default)]
    pub trajectory_memory: Option<TrajectoryMemory>,
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
            lifecycle_stage: LifecycleStage::Observed,
            lifecycle_policy: ContextLifecyclePolicy::default(),
            projections: Vec::new(),
            uncertainty: EpistemicUncertainty::default(),
            calibration: EpistemicCalibration::default(),
            attention: AttentionSignal::default(),
            context_affordance: ContextAffordance::default(),
            context_gaps: Vec::new(),
            invalidation_conditions: Vec::new(),
            trajectory_memory: None,
        })
    }

    /// Returns a copy of this cell with an updated lifecycle stage.
    pub fn with_lifecycle_stage(mut self, lifecycle_stage: LifecycleStage) -> Self {
        self.lifecycle_stage = lifecycle_stage;
        self
    }

    /// Appends a task-facing memory projection.
    pub fn add_projection(&mut self, projection: MemoryProjection) {
        self.projections.push(projection);
    }

    /// Records native context lifecycle policy metadata.
    pub fn set_lifecycle_policy(&mut self, lifecycle_policy: ContextLifecyclePolicy) {
        self.lifecycle_policy = lifecycle_policy;
    }

    /// Records native epistemic uncertainty metadata.
    pub fn set_uncertainty(&mut self, uncertainty: EpistemicUncertainty) {
        self.uncertainty = uncertainty;
    }

    /// Records native empirical calibration metadata.
    pub fn set_calibration(&mut self, calibration: EpistemicCalibration) {
        self.calibration = calibration;
    }

    /// Records native lifecycle attention metadata.
    pub fn set_attention(&mut self, attention: AttentionSignal) {
        self.attention = attention;
    }

    /// Records native value-of-context metadata.
    pub fn set_context_affordance(&mut self, context_affordance: ContextAffordance) {
        self.context_affordance = context_affordance;
    }

    /// Appends a structured missing-context contract.
    pub fn add_context_gap(&mut self, context_gap: ContextGap) {
        self.context_gaps.push(context_gap);
    }

    /// Appends a structured invalidation condition.
    pub fn add_invalidation_condition(&mut self, condition: InvalidationCondition) {
        self.invalidation_conditions.push(condition);
    }

    /// Records distilled rollout or trajectory experience for context reuse.
    pub fn set_trajectory_memory(&mut self, trajectory_memory: TrajectoryMemory) {
        self.trajectory_memory = Some(trajectory_memory);
    }

    /// Returns the deterministic epistemic action for consuming this cell.
    pub fn epistemic_action(&self) -> EpistemicAction {
        epistemic_action(self)
    }

    /// Returns deterministic reasons for the epistemic action recommendation.
    pub fn epistemic_action_reasons(&self) -> Vec<EpistemicActionReason> {
        epistemic_action_reasons(self)
    }

    /// Returns deterministic pressure signals for lifecycle context selection.
    pub fn epistemic_pressure(&self) -> EpistemicPressure {
        epistemic_pressure(self)
    }

    /// Returns deterministic lifecycle-policy evaluation for this cell.
    pub fn context_lifecycle_evaluation(&self) -> ContextLifecycleEvaluation {
        context_lifecycle_evaluation(self)
    }

    /// Returns projections matching a memory form.
    pub fn projections_by_kind(&self, kind: MemoryProjectionKind) -> Vec<&MemoryProjection> {
        self.projections
            .iter()
            .filter(|projection| projection.kind == kind)
            .collect()
    }

    /// Compiles this StateCell into a task-facing projection packet.
    pub fn context_packet(&self, profile: ContextProfile, token_budget: i64) -> ContextPacket {
        let mut selected = Vec::new();
        let mut entries = Vec::new();
        let mut token_count = 0;
        if self.calibration.is_recorded() && native_calibration_priority(profile) {
            let calibration_tokens = 12;
            if token_count + calibration_tokens <= token_budget {
                token_count += calibration_tokens;
                let text = calibration_line(&self.calibration);
                selected.push(text.clone());
                entries.push(ContextPacketEntry {
                    source: ContextPacketEntrySource::NativeCalibration,
                    text,
                    confidence: self.calibration.observed_frequency,
                    token_count: calibration_tokens,
                });
            }
        }
        if self.uncertainty.is_recorded() && native_uncertainty_priority(profile) {
            let uncertainty_tokens = 12;
            if uncertainty_tokens <= token_budget {
                token_count += uncertainty_tokens;
                let text = format!(
                    "Uncertainty {:.3}, surprise {:.3} bits: {}",
                    self.uncertainty.score.value(),
                    self.uncertainty.surprise_bits,
                    self.uncertainty.rationale
                );
                selected.push(text.clone());
                entries.push(ContextPacketEntry {
                    source: ContextPacketEntrySource::NativeUncertainty,
                    text,
                    confidence: self.uncertainty.score,
                    token_count: uncertainty_tokens,
                });
            }
            if let Some(expectation) = &self.uncertainty.expectation {
                let expectation_tokens = 6;
                if token_count + expectation_tokens <= token_budget {
                    token_count += expectation_tokens;
                    let text = format!(
                        "Expectation \"{}\" prior {:.3}, observed probability {:.3}, probability delta {:.3}",
                        expectation.label,
                        expectation.prior_probability.value(),
                        expectation.observed_probability.value(),
                        expectation.probability_delta
                    );
                    selected.push(text.clone());
                    entries.push(ContextPacketEntry {
                        source: ContextPacketEntrySource::NativeExpectation,
                        text,
                        confidence: expectation.observed_probability,
                        token_count: expectation_tokens,
                    });
                }
            }
        }
        if self.attention.is_recorded() && native_attention_priority(profile) {
            let attention_tokens = 10;
            if token_count + attention_tokens <= token_budget {
                token_count += attention_tokens;
                let text = format!(
                    "Attention salience {:.3}: novelty {:.3}, urgency {:.3}, impact {:.3}, decay resistance {:.3}",
                    self.attention.salience_score(),
                    self.attention.novelty,
                    self.attention.urgency,
                    self.attention.impact,
                    self.attention.decay_resistance
                );
                selected.push(text.clone());
                entries.push(ContextPacketEntry {
                    source: ContextPacketEntrySource::NativeAttention,
                    text,
                    confidence: bounded_entry_confidence(self.attention.salience_score()),
                    token_count: attention_tokens,
                });
            }
        }
        if self.context_affordance.is_recorded() && native_context_affordance_priority(profile) {
            let affordance_tokens = 12;
            if token_count + affordance_tokens <= token_budget {
                token_count += affordance_tokens;
                let text = context_affordance_line(&self.context_affordance);
                selected.push(text.clone());
                entries.push(ContextPacketEntry {
                    source: ContextPacketEntrySource::NativeContextAffordance,
                    text,
                    confidence: bounded_entry_confidence(
                        self.context_affordance.context_affordance_score(),
                    ),
                    token_count: affordance_tokens,
                });
            }
        }
        for gap in self
            .context_gaps
            .iter()
            .filter(|_| native_context_gap_priority(profile))
        {
            let gap_tokens = 12;
            if token_count + gap_tokens <= token_budget {
                token_count += gap_tokens;
                let text = context_gap_line(gap);
                selected.push(text.clone());
                entries.push(ContextPacketEntry {
                    source: ContextPacketEntrySource::NativeContextGap,
                    text,
                    confidence: gap.priority,
                    token_count: gap_tokens,
                });
            }
        }
        for condition in self
            .invalidation_conditions
            .iter()
            .filter(|_| native_invalidation_condition_priority(profile))
        {
            let invalidation_tokens = 12;
            if token_count + invalidation_tokens <= token_budget {
                token_count += invalidation_tokens;
                let text = invalidation_condition_line(condition);
                selected.push(text.clone());
                entries.push(ContextPacketEntry {
                    source: ContextPacketEntrySource::NativeInvalidationCondition,
                    text,
                    confidence: condition.priority,
                    token_count: invalidation_tokens,
                });
            }
        }
        if let Some(trajectory_memory) = &self.trajectory_memory {
            if native_trajectory_memory_priority(profile) {
                let trajectory_tokens = 16;
                if token_count + trajectory_tokens <= token_budget {
                    token_count += trajectory_tokens;
                    let text = trajectory_memory_line(trajectory_memory);
                    selected.push(text.clone());
                    entries.push(ContextPacketEntry {
                        source: ContextPacketEntrySource::NativeTrajectoryMemory,
                        text,
                        confidence: trajectory_memory.confidence,
                        token_count: trajectory_tokens,
                    });
                }
            }
        }
        let lifecycle_evaluation = context_lifecycle_evaluation(self);
        let included_lifecycle_policy = lifecycle_evaluation
            != ContextLifecycleEvaluation::default()
            && native_lifecycle_policy_priority(profile);
        if included_lifecycle_policy {
            let policy_tokens = 10;
            if token_count + policy_tokens <= token_budget {
                token_count += policy_tokens;
                let text = lifecycle_policy_line(&lifecycle_evaluation);
                selected.push(text.clone());
                entries.push(ContextPacketEntry {
                    source: ContextPacketEntrySource::NativeLifecyclePolicy,
                    text,
                    confidence: max_evidence_confidence(&self.evidence),
                    token_count: policy_tokens,
                });
            }
        }
        let mut included_projection = false;
        for kind in projection_priority(profile) {
            for projection in self.projections_by_kind(*kind) {
                let next_total = token_count + projection.cost.token_count;
                if next_total <= token_budget {
                    included_projection = true;
                    token_count = next_total;
                    selected.push(projection.text.clone());
                    entries.push(ContextPacketEntry {
                        source: ContextPacketEntrySource::Projection(projection.kind),
                        text: projection.text.clone(),
                        confidence: projection.confidence,
                        token_count: projection.cost.token_count,
                    });
                }
            }
        }

        let mut citations = self
            .evidence
            .iter()
            .map(|evidence| evidence.citation.locator.clone())
            .collect::<Vec<_>>();
        if let Some(trajectory_memory) = &self.trajectory_memory {
            if !trajectory_memory.trace_locator.trim().is_empty()
                && !citations.contains(&trajectory_memory.trace_locator)
            {
                citations.push(trajectory_memory.trace_locator.clone());
            }
        }

        let plan = context_packet_plan(self, profile, &entries, &citations, &lifecycle_evaluation);

        ContextPacket {
            profile,
            strategy: ContextPacketStrategy::RawProjection,
            compiler_policy: ContextCompilerPolicy::RawBaseline,
            abstraction_level: ContextAbstractionLevel::Raw,
            compiler_reason_tags: Vec::new(),
            compiler_evidence_locators: Vec::new(),
            plan,
            origin: Some(ContextPacketOrigin {
                cell_id: self.id,
                anchors: self.anchors.clone(),
                scope: Some(self.scope.clone()),
                valid_time: Some(self.valid_time.clone()),
                system_time: Some(self.system_time.clone()),
                lifecycle_stage: self.lifecycle_stage,
                activation: self.activation,
                commit_id: self.commit_id,
            }),
            selection: Some(ContextPacketSelection {
                max_confidence: max_evidence_confidence(&self.evidence),
                epistemic_action: epistemic_action(self),
                epistemic_action_reasons: epistemic_action_reasons(self),
                epistemic_pressure: epistemic_pressure(self),
                utility_score: self.utility_feedback.utility_score(),
                uncertainty_score: self.uncertainty.score,
                surprise_bits: self.uncertainty.surprise_bits,
                expectation: self.uncertainty.expectation.clone(),
                calibration: self.calibration.clone(),
                calibration_error: self.calibration.calibration_error,
                attention: self.attention,
                salience_score: self.attention.salience_score(),
                context_affordance: self.context_affordance,
                context_gaps: self.context_gaps.clone(),
                invalidation_conditions: self.invalidation_conditions.clone(),
                trajectory_memory: self.trajectory_memory.clone(),
                context_affordance_score: self.context_affordance.context_affordance_score(),
                lifecycle_policy: lifecycle_evaluation,
                answerability_questions: self.answerability.questions().to_vec(),
                reasons: context_packet_selection_reasons(included_projection, &entries, self),
            }),
            lines: selected,
            entries,
            citations,
            dependency_context: Vec::new(),
            revision_context: Vec::new(),
            token_count,
        }
    }
}

fn max_evidence_confidence(evidence: &[Evidence]) -> Confidence {
    evidence
        .iter()
        .map(|evidence| evidence.confidence)
        .max_by(|left, right| left.value().total_cmp(&right.value()))
        .unwrap_or_default()
}

fn context_packet_plan(
    cell: &StateCell,
    profile: ContextProfile,
    entries: &[ContextPacketEntry],
    citations: &[String],
    lifecycle_evaluation: &ContextLifecycleEvaluation,
) -> Option<ContextPacketPlan> {
    let has_entry_source = |source| entries.iter().any(|entry| entry.source == source);
    let has_invalidation = !cell.invalidation_conditions.is_empty()
        || has_entry_source(ContextPacketEntrySource::NativeInvalidationCondition);
    let has_trajectory = cell.trajectory_memory.is_some()
        || has_entry_source(ContextPacketEntrySource::NativeTrajectoryMemory);
    let has_uncertainty = cell.uncertainty.is_recorded()
        || cell.calibration.is_recorded()
        || has_entry_source(ContextPacketEntrySource::NativeUncertainty)
        || has_entry_source(ContextPacketEntrySource::NativeExpectation)
        || has_entry_source(ContextPacketEntrySource::NativeCalibration);
    let has_lifecycle_policy = lifecycle_evaluation != &ContextLifecycleEvaluation::default()
        || has_entry_source(ContextPacketEntrySource::NativeLifecyclePolicy);
    let has_context_affordance = cell.context_affordance.is_recorded()
        || has_entry_source(ContextPacketEntrySource::NativeContextAffordance);

    let (purpose, strategy, abstraction_level) = if profile == ContextProfile::Audit {
        (
            ContextPacketPurpose::Audit,
            ContextPacketStrategy::OperationalBrief,
            ContextAbstractionLevel::EvidenceDense,
        )
    } else if has_invalidation {
        (
            ContextPacketPurpose::Falsification,
            ContextPacketStrategy::FalsificationBrief,
            ContextAbstractionLevel::Falsification,
        )
    } else if matches!(
        lifecycle_evaluation.use_policy,
        UsePolicy::DoNotUseForAnswer | UsePolicy::VerifyBeforeUse
    ) {
        (
            ContextPacketPurpose::SafetyGuidance,
            ContextPacketStrategy::FalsificationBrief,
            ContextAbstractionLevel::Falsification,
        )
    } else if has_trajectory {
        (
            ContextPacketPurpose::TrajectoryReuse,
            ContextPacketStrategy::ScavengingBrief,
            ContextAbstractionLevel::Scavenging,
        )
    } else if has_uncertainty {
        (
            ContextPacketPurpose::UncertaintyGuidance,
            ContextPacketStrategy::UncertaintyBrief,
            ContextAbstractionLevel::Brief,
        )
    } else if has_lifecycle_policy {
        (
            ContextPacketPurpose::RevisionGuidance,
            ContextPacketStrategy::RevisionCapsule,
            ContextAbstractionLevel::Capsule,
        )
    } else {
        (
            ContextPacketPurpose::AnswerSupport,
            ContextPacketStrategy::RawProjection,
            ContextAbstractionLevel::Raw,
        )
    };

    let mut required_contracts = vec![ContextPacketRequirement::EvidenceCitations];
    let mut optional_contracts = Vec::new();
    if has_uncertainty {
        push_packet_requirement(
            contract_bucket_for(
                purpose == ContextPacketPurpose::UncertaintyGuidance
                    || purpose == ContextPacketPurpose::Audit,
                &mut required_contracts,
                &mut optional_contracts,
            ),
            ContextPacketRequirement::Uncertainty,
        );
    }
    if has_invalidation {
        push_packet_requirement(
            contract_bucket_for(
                matches!(
                    purpose,
                    ContextPacketPurpose::Falsification
                        | ContextPacketPurpose::SafetyGuidance
                        | ContextPacketPurpose::Audit
                ),
                &mut required_contracts,
                &mut optional_contracts,
            ),
            ContextPacketRequirement::Invalidation,
        );
    }
    if has_trajectory {
        push_packet_requirement(
            contract_bucket_for(
                purpose == ContextPacketPurpose::TrajectoryReuse
                    || purpose == ContextPacketPurpose::Audit,
                &mut required_contracts,
                &mut optional_contracts,
            ),
            ContextPacketRequirement::TrajectoryApplicability,
        );
    }
    if has_lifecycle_policy {
        push_packet_requirement(
            contract_bucket_for(
                matches!(
                    purpose,
                    ContextPacketPurpose::RevisionGuidance
                        | ContextPacketPurpose::SafetyGuidance
                        | ContextPacketPurpose::Audit
                ),
                &mut required_contracts,
                &mut optional_contracts,
            ),
            ContextPacketRequirement::LifecyclePolicy,
        );
    }
    if has_context_affordance {
        push_packet_requirement(
            contract_bucket_for(
                purpose == ContextPacketPurpose::Audit,
                &mut required_contracts,
                &mut optional_contracts,
            ),
            ContextPacketRequirement::ContextAffordance,
        );
    }

    let mut reason_tags = vec![
        context_packet_purpose_tag(purpose).to_string(),
        context_profile_tag(profile).to_string(),
        "evidence-citations".to_string(),
    ];
    if has_uncertainty {
        reason_tags.push("native-uncertainty".to_string());
    }
    if has_invalidation {
        reason_tags.push("invalidation-condition".to_string());
    }
    if has_trajectory {
        reason_tags.push("trajectory-memory".to_string());
    }
    if has_lifecycle_policy {
        reason_tags.push("lifecycle-policy".to_string());
    }
    if has_context_affordance {
        reason_tags.push("context-affordance".to_string());
    }

    ContextPacketPlan::new(
        purpose,
        required_contracts,
        optional_contracts,
        abstraction_level,
        strategy,
        reason_tags,
        citations.to_vec(),
    )
    .ok()
}

fn contract_bucket_for<'a>(
    required: bool,
    required_contracts: &'a mut Vec<ContextPacketRequirement>,
    optional_contracts: &'a mut Vec<ContextPacketRequirement>,
) -> &'a mut Vec<ContextPacketRequirement> {
    if required {
        required_contracts
    } else {
        optional_contracts
    }
}

fn push_packet_requirement(
    requirements: &mut Vec<ContextPacketRequirement>,
    requirement: ContextPacketRequirement,
) {
    if !requirements.contains(&requirement) {
        requirements.push(requirement);
    }
}

fn context_packet_purpose_tag(purpose: ContextPacketPurpose) -> &'static str {
    match purpose {
        ContextPacketPurpose::AnswerSupport => "answer-support",
        ContextPacketPurpose::RevisionGuidance => "revision-guidance",
        ContextPacketPurpose::SafetyGuidance => "safety-guidance",
        ContextPacketPurpose::TrajectoryReuse => "trajectory-reuse",
        ContextPacketPurpose::UncertaintyGuidance => "uncertainty-guidance",
        ContextPacketPurpose::Falsification => "falsification",
        ContextPacketPurpose::Audit => "audit",
    }
}

fn context_profile_tag(profile: ContextProfile) -> &'static str {
    match profile {
        ContextProfile::Debugging => "profile-debugging",
        ContextProfile::Planning => "profile-planning",
        ContextProfile::Execution => "profile-execution",
        ContextProfile::Audit => "profile-audit",
        ContextProfile::Reflection => "profile-reflection",
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct IncludedNativePacketEntries {
    calibration: bool,
    uncertainty: bool,
    context_affordance: bool,
    lifecycle_policy: bool,
    context_gap: bool,
    invalidation_condition: bool,
    trajectory_memory: bool,
}

impl IncludedNativePacketEntries {
    fn from_entries(entries: &[ContextPacketEntry]) -> Self {
        Self {
            calibration: entries
                .iter()
                .any(|entry| entry.source == ContextPacketEntrySource::NativeCalibration),
            uncertainty: entries
                .iter()
                .any(|entry| entry.source == ContextPacketEntrySource::NativeUncertainty),
            context_affordance: entries
                .iter()
                .any(|entry| entry.source == ContextPacketEntrySource::NativeContextAffordance),
            lifecycle_policy: entries
                .iter()
                .any(|entry| entry.source == ContextPacketEntrySource::NativeLifecyclePolicy),
            context_gap: entries
                .iter()
                .any(|entry| entry.source == ContextPacketEntrySource::NativeContextGap),
            invalidation_condition: entries
                .iter()
                .any(|entry| entry.source == ContextPacketEntrySource::NativeInvalidationCondition),
            trajectory_memory: entries
                .iter()
                .any(|entry| entry.source == ContextPacketEntrySource::NativeTrajectoryMemory),
        }
    }
}

fn context_packet_selection_reasons(
    included_projection: bool,
    entries: &[ContextPacketEntry],
    cell: &StateCell,
) -> Vec<ContextPacketSelectionReason> {
    let included = IncludedNativePacketEntries::from_entries(entries);
    let mut reasons = Vec::new();
    if max_evidence_confidence(&cell.evidence).value() > 0.0 {
        reasons.push(ContextPacketSelectionReason::EvidenceConfidence);
    }
    if cell.utility_feedback.utility_score() != UtilityFeedback::default().utility_score() {
        reasons.push(ContextPacketSelectionReason::UtilityFeedback);
    }
    if cell.lifecycle_stage != LifecycleStage::Observed {
        reasons.push(ContextPacketSelectionReason::LifecycleStage);
    }
    if included_projection {
        reasons.push(ContextPacketSelectionReason::ProjectionProfile);
    }
    if included.calibration {
        reasons.push(ContextPacketSelectionReason::EpistemicCalibration);
    }
    if included.uncertainty {
        reasons.push(ContextPacketSelectionReason::NativeUncertainty);
    }
    if included.context_affordance {
        reasons.push(ContextPacketSelectionReason::ContextAffordance);
    }
    if included.lifecycle_policy {
        reasons.push(ContextPacketSelectionReason::LifecyclePolicy);
    }
    if included.context_gap {
        reasons.push(ContextPacketSelectionReason::ContextGap);
    }
    if included.invalidation_condition {
        reasons.push(ContextPacketSelectionReason::InvalidationCondition);
    }
    if included.trajectory_memory {
        reasons.push(ContextPacketSelectionReason::TrajectoryMemory);
    }
    if cell.attention.is_recorded() {
        reasons.push(ContextPacketSelectionReason::AttentionSignal);
    }
    if !cell.answerability.questions().is_empty() {
        reasons.push(ContextPacketSelectionReason::Answerability);
    }
    reasons
}

fn context_lifecycle_evaluation(cell: &StateCell) -> ContextLifecycleEvaluation {
    let mut reasons = Vec::new();
    match cell.lifecycle_policy.retention {
        RetentionPolicy::Persistent => reasons.push(ContextLifecycleReason::PersistentRetention),
        RetentionPolicy::DecayUnlessReinforced => {
            reasons.push(ContextLifecycleReason::DecayUnlessReinforced);
        }
        RetentionPolicy::Ephemeral => reasons.push(ContextLifecycleReason::EphemeralRetention),
    }

    match cell.lifecycle_policy.use_policy {
        UsePolicy::UseDirectly => reasons.push(ContextLifecycleReason::UseDirectly),
        UsePolicy::HedgeBeforeUse => reasons.push(ContextLifecycleReason::HedgeBeforeUse),
        UsePolicy::VerifyBeforeUse => reasons.push(ContextLifecycleReason::VerifyBeforeUse),
        UsePolicy::DoNotUseForAnswer => reasons.push(ContextLifecycleReason::DoNotUseForAnswer),
    }

    match cell.lifecycle_policy.promotion {
        PromotionPolicy::Manual => reasons.push(ContextLifecycleReason::ManualPromotion),
        PromotionPolicy::EvidenceCount(minimum) | PromotionPolicy::RepeatedObservation(minimum) => {
            let reason = if cell.evidence.len() >= minimum {
                ContextLifecycleReason::PromotionEvidenceCountMet
            } else {
                ContextLifecycleReason::PromotionEvidenceCountMissing
            };
            reasons.push(reason);
        }
        PromotionPolicy::ConfidenceThreshold(minimum) => {
            let reason = if max_evidence_confidence(&cell.evidence).value() >= minimum.value() {
                ContextLifecycleReason::PromotionThresholdMet
            } else {
                ContextLifecycleReason::PromotionThresholdMissing
            };
            reasons.push(reason);
        }
    }

    if matches!(
        cell.lifecycle_policy.promotion,
        PromotionPolicy::RepeatedObservation(_)
    ) && !reasons.contains(&ContextLifecycleReason::PromotionEvidenceCountMet)
    {
        reasons.push(ContextLifecycleReason::RepeatedObservationRequired);
    }

    ContextLifecycleEvaluation {
        retention: cell.lifecycle_policy.retention,
        use_policy: cell.lifecycle_policy.use_policy,
        promotion: cell.lifecycle_policy.promotion,
        reasons,
    }
}

fn epistemic_action(cell: &StateCell) -> EpistemicAction {
    let reasons = epistemic_action_reasons(cell);
    if reasons.contains(&EpistemicActionReason::HighUncertainty)
        && reasons.contains(&EpistemicActionReason::HighSurprise)
    {
        return EpistemicAction::Scavenge;
    }
    if reasons.contains(&EpistemicActionReason::HighUncertainty) {
        return EpistemicAction::Verify;
    }
    if reasons.contains(&EpistemicActionReason::HighSurprise) {
        return EpistemicAction::Revise;
    }
    if reasons.contains(&EpistemicActionReason::MiscalibratedConfidence) {
        return EpistemicAction::Verify;
    }
    if reasons.contains(&EpistemicActionReason::ModerateUncertainty)
        || reasons.contains(&EpistemicActionReason::LowEvidenceConfidence)
    {
        return EpistemicAction::Hedge;
    }
    EpistemicAction::Use
}

fn epistemic_action_reasons(cell: &StateCell) -> Vec<EpistemicActionReason> {
    let mut reasons = Vec::new();
    let uncertainty = cell.uncertainty.score.value();
    let surprise = cell.uncertainty.surprise_bits;
    if cell.uncertainty.is_recorded() {
        if uncertainty >= 0.7 {
            reasons.push(EpistemicActionReason::HighUncertainty);
        } else if uncertainty >= 0.3 {
            reasons.push(EpistemicActionReason::ModerateUncertainty);
        }
        if surprise >= 2.0 {
            reasons.push(EpistemicActionReason::HighSurprise);
        }
    }
    if cell.calibration.is_recorded() && cell.calibration.calibration_error >= 0.3 {
        reasons.push(EpistemicActionReason::MiscalibratedConfidence);
    }
    if max_evidence_confidence(&cell.evidence).value() < 0.6 {
        reasons.push(EpistemicActionReason::LowEvidenceConfidence);
    }
    reasons
}

fn epistemic_pressure(cell: &StateCell) -> EpistemicPressure {
    let confidence = max_evidence_confidence(&cell.evidence).value();
    let surprise_pressure = (cell.uncertainty.surprise_bits / 4.0).clamp(0.0, 1.0);
    let impact = if cell.attention.is_recorded() {
        cell.attention.impact
    } else {
        1.0
    };
    let revision_pressure = confidence * surprise_pressure * impact;
    let scavenging_pressure =
        cell.uncertainty.score.value() * (0.5 + (0.5 * cell.attention.salience_score()));
    EpistemicPressure {
        revision_pressure,
        scavenging_pressure,
        checkout_pressure: revision_pressure.max(scavenging_pressure),
    }
}

fn native_uncertainty_priority(profile: ContextProfile) -> bool {
    matches!(
        profile,
        ContextProfile::Debugging | ContextProfile::Planning | ContextProfile::Audit
    )
}

fn native_calibration_priority(profile: ContextProfile) -> bool {
    matches!(
        profile,
        ContextProfile::Debugging
            | ContextProfile::Planning
            | ContextProfile::Execution
            | ContextProfile::Audit
            | ContextProfile::Reflection
    )
}

fn native_attention_priority(profile: ContextProfile) -> bool {
    matches!(
        profile,
        ContextProfile::Debugging
            | ContextProfile::Planning
            | ContextProfile::Execution
            | ContextProfile::Reflection
    )
}

fn native_lifecycle_policy_priority(profile: ContextProfile) -> bool {
    matches!(
        profile,
        ContextProfile::Debugging
            | ContextProfile::Planning
            | ContextProfile::Execution
            | ContextProfile::Audit
    )
}

fn native_context_affordance_priority(profile: ContextProfile) -> bool {
    matches!(
        profile,
        ContextProfile::Debugging
            | ContextProfile::Planning
            | ContextProfile::Execution
            | ContextProfile::Audit
            | ContextProfile::Reflection
    )
}

fn native_context_gap_priority(profile: ContextProfile) -> bool {
    matches!(
        profile,
        ContextProfile::Debugging
            | ContextProfile::Planning
            | ContextProfile::Execution
            | ContextProfile::Audit
            | ContextProfile::Reflection
    )
}

fn native_invalidation_condition_priority(profile: ContextProfile) -> bool {
    matches!(
        profile,
        ContextProfile::Debugging
            | ContextProfile::Planning
            | ContextProfile::Execution
            | ContextProfile::Audit
            | ContextProfile::Reflection
    )
}

fn native_trajectory_memory_priority(profile: ContextProfile) -> bool {
    matches!(
        profile,
        ContextProfile::Debugging
            | ContextProfile::Planning
            | ContextProfile::Execution
            | ContextProfile::Reflection
    )
}

fn calibration_line(calibration: &EpistemicCalibration) -> String {
    format!(
        "Calibration expected confidence {:.3}, observed frequency {:.3}, error {:.3} over {} samples: {}",
        calibration.expected_confidence.value(),
        calibration.observed_frequency.value(),
        calibration.calibration_error,
        calibration.sample_count,
        calibration.rationale
    )
}

fn context_affordance_line(affordance: &ContextAffordance) -> String {
    format!(
        "Context affordance {:.3}: task value {:.3}, information gain {:.3}, misuse risk {:.3}, ambiguity {:.3}, applicability {:.3}, resource pressure {:.3}",
        affordance.context_affordance_score(),
        affordance.expected_task_value,
        affordance.expected_information_gain,
        affordance.risk_of_misuse,
        affordance.ambiguity,
        affordance.applicability,
        affordance.resource_pressure
    )
}

fn bounded_entry_confidence(score: f32) -> Confidence {
    match Confidence::new(score) {
        Ok(confidence) => confidence,
        Err(_) => Confidence::ZERO,
    }
}

fn context_gap_line(gap: &ContextGap) -> String {
    format!(
        "Context gap {} priority {:.3}: {} Rationale: {}",
        context_gap_kind_text(gap.kind),
        gap.priority.value(),
        gap.question,
        gap.rationale
    )
}

fn context_gap_kind_text(kind: ContextGapKind) -> &'static str {
    match kind {
        ContextGapKind::MissingEvidence => "missing evidence",
        ContextGapKind::MissingDecision => "missing decision",
        ContextGapKind::MissingConstraint => "missing constraint",
        ContextGapKind::MissingDependency => "missing dependency",
    }
}

fn invalidation_condition_line(condition: &InvalidationCondition) -> String {
    format!(
        "Invalidation condition {} priority {:.3}: {} Rationale: {}",
        invalidation_condition_kind_text(condition.kind),
        condition.priority.value(),
        condition.condition,
        condition.rationale
    )
}

fn invalidation_condition_kind_text(kind: InvalidationConditionKind) -> &'static str {
    match kind {
        InvalidationConditionKind::ContradictoryEvidence => "contradictory evidence",
        InvalidationConditionKind::BoundaryViolation => "boundary violation",
        InvalidationConditionKind::TemporalExpiry => "temporal expiry",
        InvalidationConditionKind::DependencyInvalidated => "dependency invalidated",
    }
}

fn trajectory_memory_line(memory: &TrajectoryMemory) -> String {
    format!(
        "Trajectory lesson confidence {:.3}: {}. Tried: {}. Progress: {}. Failure: {}. Applies when: {}. Invalidated when: {}. Trace: {}",
        memory.confidence.value(),
        memory.reusable_lesson,
        memory.hypothesis_tried,
        memory.progress_made,
        memory.failure_mode,
        memory.applicability_conditions.join("; "),
        memory.invalidation_conditions.join("; "),
        memory.trace_locator
    )
}

fn lifecycle_policy_line(evaluation: &ContextLifecycleEvaluation) -> String {
    format!(
        "Lifecycle policy: {}; {}; {}",
        retention_policy_text(evaluation.retention),
        use_policy_text(evaluation.use_policy),
        promotion_policy_text(evaluation.promotion)
    )
}

fn retention_policy_text(policy: RetentionPolicy) -> &'static str {
    match policy {
        RetentionPolicy::Persistent => "persistent retention",
        RetentionPolicy::DecayUnlessReinforced => "decay unless reinforced",
        RetentionPolicy::Ephemeral => "ephemeral retention",
    }
}

fn use_policy_text(policy: UsePolicy) -> &'static str {
    match policy {
        UsePolicy::UseDirectly => "use directly",
        UsePolicy::HedgeBeforeUse => "hedge before use",
        UsePolicy::VerifyBeforeUse => "verify before use",
        UsePolicy::DoNotUseForAnswer => "do not use for answer",
    }
}

fn promotion_policy_text(policy: PromotionPolicy) -> String {
    match policy {
        PromotionPolicy::Manual => "manual promotion".to_string(),
        PromotionPolicy::EvidenceCount(minimum) => {
            format!("promote after {minimum} evidence records")
        }
        PromotionPolicy::ConfidenceThreshold(minimum) => {
            format!("promote at confidence {:.3}", minimum.value())
        }
        PromotionPolicy::RepeatedObservation(minimum) => {
            format!("promote after {minimum} repeated observations")
        }
    }
}

fn surprise_bits(probability: f32) -> f32 {
    let bounded_probability =
        probability.max(EpistemicExpectation::MIN_OBSERVED_PROBABILITY_FOR_SURPRISE);
    -bounded_probability.log2()
}

fn round_probability(probability: f32) -> f32 {
    (probability * 1_000_000.0).round() / 1_000_000.0
}

fn probability_delta(prior_probability: Confidence, observed_probability: Confidence) -> f32 {
    round_probability((prior_probability.value() - observed_probability.value()).abs())
}

fn validate_attention_component(value: f32) -> Result<(), CoreError> {
    if !(0.0..=1.0).contains(&value) {
        return Err(CoreError::AttentionSignalOutOfRange { value });
    }

    Ok(())
}

fn validate_context_affordance_component(value: f32) -> Result<(), CoreError> {
    if !(0.0..=1.0).contains(&value) {
        return Err(CoreError::ContextAffordanceOutOfRange { value });
    }

    Ok(())
}

fn validate_context_gap_priority(value: f32) -> Result<(), CoreError> {
    if !(0.0..=1.0).contains(&value) || !value.is_finite() {
        return Err(CoreError::ContextGapPriorityOutOfRange { value });
    }

    Ok(())
}

fn validate_invalidation_priority(value: f32) -> Result<(), CoreError> {
    if !(0.0..=1.0).contains(&value) || !value.is_finite() {
        return Err(CoreError::InvalidationPriorityOutOfRange { value });
    }

    Ok(())
}

fn projection_priority(profile: ContextProfile) -> &'static [MemoryProjectionKind] {
    match profile {
        ContextProfile::Debugging => &[
            MemoryProjectionKind::Episodic,
            MemoryProjectionKind::Uncertainty,
            MemoryProjectionKind::Semantic,
            MemoryProjectionKind::Procedural,
            MemoryProjectionKind::Policy,
        ],
        ContextProfile::Planning => &[
            MemoryProjectionKind::Semantic,
            MemoryProjectionKind::Uncertainty,
            MemoryProjectionKind::Procedural,
            MemoryProjectionKind::Episodic,
            MemoryProjectionKind::Policy,
        ],
        ContextProfile::Execution => &[
            MemoryProjectionKind::Semantic,
            MemoryProjectionKind::Procedural,
            MemoryProjectionKind::Policy,
            MemoryProjectionKind::Uncertainty,
            MemoryProjectionKind::Episodic,
        ],
        ContextProfile::Audit => &[
            MemoryProjectionKind::Episodic,
            MemoryProjectionKind::Semantic,
            MemoryProjectionKind::Uncertainty,
            MemoryProjectionKind::Procedural,
            MemoryProjectionKind::Policy,
        ],
        ContextProfile::Reflection => &[
            MemoryProjectionKind::Episodic,
            MemoryProjectionKind::Semantic,
            MemoryProjectionKind::Procedural,
            MemoryProjectionKind::Policy,
            MemoryProjectionKind::Uncertainty,
        ],
    }
}
