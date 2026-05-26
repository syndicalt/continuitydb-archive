//! Typed ContinuityDB query AST.

mod text;

use chrono::{DateTime, Utc};
use continuitydb_checkout::CheckoutRequest;
use continuitydb_core::{
    ActivationState, CellDependencyKind, CommitId, Confidence, ContextCompilerPolicy,
    ContextCompilerProposal, ContextGapKind, ContextPacketSelectionReason, ContextPacketStrategy,
    ContextProfile, EpistemicAction, EpistemicActionReason, InvalidationConditionKind,
    LifecycleStage, MemoryProjectionKind, PromotionPolicy, RetentionPolicy, RevisionLinkKind,
    Scope, SemanticAnchor, StateCellId, UsePolicy,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use text::{parse_query_text, QueryTextError};

/// Wire-format marker for versioned query JSON envelopes.
pub const QUERY_ENVELOPE_FORMAT: &str = "continuitydb.query";
/// Supported query JSON envelope version.
pub const QUERY_ENVELOPE_FORMAT_VERSION: u32 = 1;

/// Versioned JSON envelope for portable typed query files.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QueryEnvelope {
    /// Wire-format marker.
    pub format: String,
    /// Wire-format version.
    pub version: u32,
    /// Serialized typed query.
    pub query: ContinuityQuery,
}

impl QueryEnvelope {
    /// Wraps a typed query in the current JSON envelope.
    pub fn new(query: ContinuityQuery) -> Self {
        Self {
            format: QUERY_ENVELOPE_FORMAT.to_string(),
            version: QUERY_ENVELOPE_FORMAT_VERSION,
            query,
        }
    }

    /// Validates the envelope format and version.
    pub fn validate(&self) -> Result<(), QueryEnvelopeError> {
        if self.format == QUERY_ENVELOPE_FORMAT && self.version == QUERY_ENVELOPE_FORMAT_VERSION {
            Ok(())
        } else {
            Err(QueryEnvelopeError::InvalidEnvelope)
        }
    }
}

/// Top-level typed ContinuityDB query.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContinuityQuery {
    /// Materialize a continuity checkout slice.
    Checkout(CheckoutQuery),
}

impl ContinuityQuery {
    /// Compiles this query into a deterministic checkout request.
    pub fn compile_checkout(self) -> Result<CheckoutRequest, QueryError> {
        match self {
            Self::Checkout(query) => query.compile_checkout(),
        }
    }

    /// Returns the requested checkout materialization shape.
    pub fn return_shape(&self) -> QueryReturnShape {
        match self {
            Self::Checkout(query) => query.return_shape(),
        }
    }
}

/// Structured checkout query.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CheckoutQuery {
    task: QueryTask,
    requirements: QueryRequirements,
    #[serde(default)]
    compiler_policy: ContextCompilerPolicy,
    #[serde(default)]
    compiler_intent: Option<String>,
    #[serde(default)]
    compiler_proposals: Vec<ContextCompilerProposal>,
    return_shape: QueryReturnShape,
    optimization: QueryOptimization,
}

impl CheckoutQuery {
    /// Creates a checkout query with default requirements and supported output semantics.
    pub fn new(task: QueryTask) -> Self {
        Self {
            task,
            requirements: QueryRequirements::default(),
            compiler_policy: ContextCompilerPolicy::Automatic,
            compiler_intent: None,
            compiler_proposals: Vec::new(),
            return_shape: QueryReturnShape::PackedContextWithMetadata,
            optimization: QueryOptimization::DeterministicUtility,
        }
    }

    /// Returns query task identity and answerability intent.
    pub fn task(&self) -> &QueryTask {
        &self.task
    }

    /// Returns deterministic checkout requirements.
    pub fn requirements(&self) -> &QueryRequirements {
        &self.requirements
    }

    /// Returns the requested materialization shape.
    pub fn return_shape(&self) -> QueryReturnShape {
        self.return_shape
    }

    /// Returns the requested optimization policy.
    pub fn optimization(&self) -> QueryOptimization {
        self.optimization
    }

    /// Returns the requested context compiler policy.
    pub fn compiler_policy(&self) -> ContextCompilerPolicy {
        self.compiler_policy
    }

    /// Returns task intent used only to shape compiler output.
    pub fn compiler_intent(&self) -> Option<&str> {
        self.compiler_intent.as_deref()
    }

    /// Returns accepted model-assisted compiler proposals.
    pub fn compiler_proposals(&self) -> &[ContextCompilerProposal] {
        &self.compiler_proposals
    }

    /// Replaces deterministic checkout requirements.
    pub fn with_requirements(mut self, requirements: QueryRequirements) -> Self {
        self.requirements = requirements;
        self
    }

    /// Replaces the context compiler policy.
    pub fn with_compiler_policy(mut self, compiler_policy: ContextCompilerPolicy) -> Self {
        self.compiler_policy = compiler_policy;
        self
    }

    /// Sets task intent used only to shape compiler output.
    pub fn with_compiler_intent(mut self, compiler_intent: impl Into<String>) -> Self {
        self.compiler_intent = Some(compiler_intent.into());
        self
    }

    /// Sets optional task intent used only to shape compiler output.
    pub fn with_optional_compiler_intent(mut self, compiler_intent: Option<String>) -> Self {
        self.compiler_intent = compiler_intent;
        self
    }

    /// Replaces accepted model-assisted compiler proposals.
    pub fn with_compiler_proposals(
        mut self,
        compiler_proposals: Vec<ContextCompilerProposal>,
    ) -> Self {
        self.compiler_proposals = compiler_proposals;
        self
    }

    /// Replaces the requested materialization shape.
    pub fn with_return_shape(mut self, return_shape: QueryReturnShape) -> Self {
        self.return_shape = return_shape;
        self
    }

    /// Replaces the requested optimization policy.
    pub fn with_optimization(mut self, optimization: QueryOptimization) -> Self {
        self.optimization = optimization;
        self
    }

    /// Compiles this query into the current deterministic checkout request type.
    pub fn compile_checkout(self) -> Result<CheckoutRequest, QueryError> {
        if self.optimization != QueryOptimization::DeterministicUtility {
            return Err(QueryError::UnsupportedOptimization(self.optimization));
        }

        Ok(CheckoutRequest {
            semantic_anchor: self.requirements.semantic_anchor,
            scope: self.requirements.scope,
            valid_at: self.requirements.valid_at,
            system_at: self.requirements.system_at,
            commit_id: self.requirements.commit_id,
            activation: self.requirements.activation,
            lifecycle_stage: self.requirements.lifecycle_stage,
            retention_policy: self.requirements.retention_policy,
            use_policy: self.requirements.use_policy,
            promotion_policy: self.requirements.promotion_policy,
            projection_kind: self.requirements.projection_kind,
            minimum_uncertainty: self.requirements.minimum_uncertainty,
            minimum_surprise_bits: self.requirements.minimum_surprise_bits,
            minimum_probability_delta: self.requirements.minimum_probability_delta,
            minimum_salience: self.requirements.minimum_salience,
            minimum_context_affordance: self.requirements.minimum_context_affordance,
            minimum_epistemic_pressure: self.requirements.minimum_epistemic_pressure,
            context_gap_kind: self.requirements.context_gap_kind,
            minimum_context_gap_priority: self.requirements.minimum_context_gap_priority,
            invalidation_condition_kind: self.requirements.invalidation_condition_kind,
            minimum_invalidation_priority: self.requirements.minimum_invalidation_priority,
            epistemic_action: self.requirements.epistemic_action,
            epistemic_action_reason: self.requirements.epistemic_action_reason,
            selection_reason: self.requirements.selection_reason,
            trajectory_memory_strategy: self.requirements.trajectory_memory_strategy,
            minimum_trajectory_memory_confidence: self
                .requirements
                .minimum_trajectory_memory_confidence,
            answerability_question: Some(self.task.answerability_question),
            compiler_intent: self.compiler_intent,
            compiler_proposals: self.compiler_proposals,
            evidence_source: self.requirements.evidence_source,
            dependency_target: self.requirements.dependency_target,
            dependency_kind: self.requirements.dependency_kind,
            revision_related_cell: self.requirements.revision_related_cell,
            revision_link_kind: self.requirements.revision_link_kind,
            context_profile: self.requirements.context_profile,
            compiler_policy: self.compiler_policy,
            minimum_confidence: self.requirements.minimum_confidence,
            token_budget: self.requirements.token_budget,
        })
    }
}

/// Query task identity and answerability intent.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct QueryTask {
    /// Stable task name or identifier.
    pub name: String,
    /// Exact question or intent selected StateCells should answer.
    pub answerability_question: String,
}

impl QueryTask {
    /// Creates a query task.
    pub fn new(name: impl Into<String>, answerability_question: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            answerability_question: answerability_question.into(),
        }
    }
}

/// Deterministic requirements accepted by the first checkout query compiler.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QueryRequirements {
    /// Optional semantic anchor filter.
    pub semantic_anchor: Option<SemanticAnchor>,
    /// Optional scope filter.
    pub scope: Option<Scope>,
    /// Optional valid-time filter.
    pub valid_at: Option<DateTime<Utc>>,
    /// Optional system transaction-time filter.
    pub system_at: Option<DateTime<Utc>>,
    /// Optional database commit identifier filter.
    pub commit_id: Option<CommitId>,
    /// Optional activation-state filter.
    pub activation: Option<ActivationState>,
    /// Optional StateCell v2 lifecycle-stage filter.
    pub lifecycle_stage: Option<LifecycleStage>,
    /// Optional StateCell v2 lifecycle retention-policy filter.
    pub retention_policy: Option<RetentionPolicy>,
    /// Optional StateCell v2 lifecycle use-policy filter.
    pub use_policy: Option<UsePolicy>,
    /// Optional StateCell v2 lifecycle promotion-policy filter.
    pub promotion_policy: Option<PromotionPolicy>,
    /// Optional StateCell v2 memory projection kind filter.
    pub projection_kind: Option<MemoryProjectionKind>,
    /// Optional minimum native StateCell v2 uncertainty score.
    pub minimum_uncertainty: Option<Confidence>,
    /// Optional minimum native StateCell v2 surprise value in bits.
    pub minimum_surprise_bits: Option<f32>,
    /// Optional minimum native StateCell v2 expectation probability movement.
    pub minimum_probability_delta: Option<f32>,
    /// Optional minimum native StateCell v2 attention salience score.
    pub minimum_salience: Option<f32>,
    /// Optional minimum native StateCell v2 value-of-context score.
    pub minimum_context_affordance: Option<f32>,
    /// Optional minimum derived StateCell v2 epistemic pressure score.
    pub minimum_epistemic_pressure: Option<f32>,
    /// Optional StateCell v2 missing-context gap kind filter.
    pub context_gap_kind: Option<ContextGapKind>,
    /// Optional minimum StateCell v2 missing-context gap priority.
    pub minimum_context_gap_priority: Option<Confidence>,
    /// Optional StateCell v2 invalidation-condition kind filter.
    pub invalidation_condition_kind: Option<InvalidationConditionKind>,
    /// Optional minimum StateCell v2 invalidation-condition priority.
    pub minimum_invalidation_priority: Option<Confidence>,
    /// Optional deterministic StateCell v2 epistemic action filter.
    pub epistemic_action: Option<EpistemicAction>,
    /// Optional deterministic StateCell v2 epistemic action reason filter.
    pub epistemic_action_reason: Option<EpistemicActionReason>,
    /// Optional deterministic StateCell v2 packet selection reason filter.
    pub selection_reason: Option<ContextPacketSelectionReason>,
    /// Optional StateCell v2 trajectory-memory checkout strategy filter.
    pub trajectory_memory_strategy: Option<ContextPacketStrategy>,
    /// Optional minimum StateCell v2 trajectory-memory confidence filter.
    pub minimum_trajectory_memory_confidence: Option<Confidence>,
    /// Optional exact evidence-source filter.
    pub evidence_source: Option<String>,
    /// Optional dependency target filter.
    pub dependency_target: Option<StateCellId>,
    /// Optional dependency kind filter.
    pub dependency_kind: Option<CellDependencyKind>,
    /// Optional related StateCell that selected cells must be connected to by a revision link.
    pub revision_related_cell: Option<StateCellId>,
    /// Optional revision-link kind selected cells must participate in.
    pub revision_link_kind: Option<RevisionLinkKind>,
    /// Projection profile used to compile selected cells into model-facing context packets.
    pub context_profile: ContextProfile,
    /// Minimum evidence confidence for included cells.
    pub minimum_confidence: Confidence,
    /// Maximum token budget for the returned slice.
    pub token_budget: i64,
}

impl Default for QueryRequirements {
    fn default() -> Self {
        Self {
            semantic_anchor: None,
            scope: None,
            valid_at: None,
            system_at: None,
            commit_id: None,
            activation: None,
            lifecycle_stage: None,
            retention_policy: None,
            use_policy: None,
            promotion_policy: None,
            projection_kind: None,
            minimum_uncertainty: None,
            minimum_surprise_bits: None,
            minimum_probability_delta: None,
            minimum_salience: None,
            minimum_context_affordance: None,
            minimum_epistemic_pressure: None,
            context_gap_kind: None,
            minimum_context_gap_priority: None,
            invalidation_condition_kind: None,
            minimum_invalidation_priority: None,
            epistemic_action: None,
            epistemic_action_reason: None,
            selection_reason: None,
            trajectory_memory_strategy: None,
            minimum_trajectory_memory_confidence: None,
            evidence_source: None,
            dependency_target: None,
            dependency_kind: None,
            revision_related_cell: None,
            revision_link_kind: None,
            context_profile: ContextProfile::Execution,
            minimum_confidence: zero_confidence(),
            token_budget: i64::MAX,
        }
    }
}

fn zero_confidence() -> Confidence {
    Confidence::new(0.0).unwrap_or_default()
}

/// Requested checkout materialization shape.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueryReturnShape {
    /// Current full checkout slice shape.
    PackedContextWithMetadata,
    /// Deterministic aggregate summary without full StateCell payload projection.
    SummaryOnly,
    /// Selected StateCell payloads without checkout metadata.
    CellsOnly,
    /// Compiled StateCell v2 context packets without full cell payloads or checkout metadata.
    ContextPacketsOnly,
}

/// Requested checkout optimization policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueryOptimization {
    /// Current deterministic utility-aware ranking.
    DeterministicUtility,
    /// Future cost-only packing.
    TokenCostOnly,
}

/// Query compilation errors.
#[derive(Debug, Error, PartialEq)]
pub enum QueryError {
    /// The requested return shape is not supported by this compiler.
    #[error("unsupported return shape: {0:?}")]
    UnsupportedReturnShape(QueryReturnShape),
    /// The requested optimization policy is not supported by this compiler.
    #[error("unsupported optimization: {0:?}")]
    UnsupportedOptimization(QueryOptimization),
}

/// Query envelope encoding and decoding errors.
#[derive(Debug, Error, PartialEq)]
pub enum QueryEnvelopeError {
    /// Query envelope JSON could not be encoded or decoded.
    #[error("query envelope JSON is invalid")]
    InvalidJson,
    /// Query envelope has an unsupported format or version.
    #[error("query envelope is invalid")]
    InvalidEnvelope,
}

/// Encodes a typed query as a versioned JSON envelope.
pub fn encode_query_json(query: ContinuityQuery) -> Result<Vec<u8>, QueryEnvelopeError> {
    serde_json::to_vec(&QueryEnvelope::new(query)).map_err(|_error| QueryEnvelopeError::InvalidJson)
}

/// Decodes a typed query from a versioned JSON envelope.
pub fn decode_query_json(bytes: &[u8]) -> Result<ContinuityQuery, QueryEnvelopeError> {
    let envelope = serde_json::from_slice::<QueryEnvelope>(bytes)
        .map_err(|_error| QueryEnvelopeError::InvalidJson)?;
    envelope.validate()?;
    Ok(envelope.query)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use continuitydb_core::{
        ActivationState, CellDependencyKind, CommitId, Confidence, ContextAbstractionLevel,
        ContextPacketStrategy, ContextProfile, LifecycleStage, MemoryProjectionKind,
        RevisionLinkKind, Scope, SemanticAnchor, StateCellId,
    };

    #[test]
    fn minimal_checkout_query_compiles_task_answerability_and_defaults(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let task = QueryTask::new("release-readiness", "what is the release status?");
        let request = CheckoutQuery::new(task).compile_checkout()?;

        assert_eq!(
            request.answerability_question.as_deref(),
            Some("what is the release status?")
        );
        assert_eq!(request.semantic_anchor, None);
        assert_eq!(request.scope, None);
        assert_eq!(request.valid_at, None);
        assert_eq!(request.system_at, None);
        assert_eq!(request.commit_id, None);
        assert_eq!(request.activation, None);
        assert_eq!(request.evidence_source, None);
        assert_eq!(request.dependency_target, None);
        assert_eq!(request.dependency_kind, None);
        assert_eq!(request.revision_related_cell, None);
        assert_eq!(request.revision_link_kind, None);
        assert_eq!(request.minimum_confidence, Confidence::new(0.0)?);
        assert_eq!(request.token_budget, i64::MAX);
        Ok(())
    }

    #[test]
    fn checkout_query_compiles_temporal_confidence_scope_and_budget_requirements(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let valid_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 10, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let system_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 11, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let commit_id = CommitId::new();
        let task = QueryTask::new("release-readiness", "what is the release status?");
        let requirements = QueryRequirements {
            scope: Some(Scope::Project("continuitydb".to_string())),
            valid_at: Some(valid_at),
            system_at: Some(system_at),
            commit_id: Some(commit_id),
            minimum_confidence: Confidence::new(0.7)?,
            token_budget: 1200,
            ..QueryRequirements::default()
        };

        let request = CheckoutQuery::new(task)
            .with_requirements(requirements)
            .compile_checkout()?;

        assert_eq!(
            request.scope,
            Some(Scope::Project("continuitydb".to_string()))
        );
        assert_eq!(request.valid_at, Some(valid_at));
        assert_eq!(request.system_at, Some(system_at));
        assert_eq!(request.commit_id, Some(commit_id));
        assert_eq!(request.minimum_confidence, Confidence::new(0.7)?);
        assert_eq!(request.token_budget, 1200);
        Ok(())
    }

    #[test]
    fn checkout_query_compiles_evidence_and_dependency_requirements(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let dependency_target = StateCellId::new();
        let task = QueryTask::new("audit-risk", "what risks depend on this evidence?");
        let requirements = QueryRequirements {
            evidence_source: Some("source:incident-review".to_string()),
            dependency_target: Some(dependency_target),
            dependency_kind: Some(CellDependencyKind::DependsOn),
            ..QueryRequirements::default()
        };

        let request = CheckoutQuery::new(task)
            .with_requirements(requirements)
            .compile_checkout()?;

        assert_eq!(
            request.evidence_source.as_deref(),
            Some("source:incident-review")
        );
        assert_eq!(request.dependency_target, Some(dependency_target));
        assert_eq!(request.dependency_kind, Some(CellDependencyKind::DependsOn));
        Ok(())
    }

    #[test]
    fn checkout_query_compiles_revision_link_requirements() -> Result<(), Box<dyn std::error::Error>>
    {
        let related_cell = StateCellId::from_u128(42);
        let task = QueryTask::new("conflict-review", "what conflicts with this cell?");
        let requirements = QueryRequirements {
            revision_related_cell: Some(related_cell),
            revision_link_kind: Some(RevisionLinkKind::ConflictsWith),
            ..QueryRequirements::default()
        };

        let request = CheckoutQuery::new(task)
            .with_requirements(requirements)
            .compile_checkout()?;

        assert_eq!(request.revision_related_cell, Some(related_cell));
        assert_eq!(
            request.revision_link_kind,
            Some(RevisionLinkKind::ConflictsWith)
        );
        Ok(())
    }

    #[test]
    fn checkout_query_compiles_activation_requirement() -> Result<(), Box<dyn std::error::Error>> {
        let task = QueryTask::new("frontier-review", "what frontier cells need review?");
        let requirements = QueryRequirements {
            activation: Some(ActivationState::Frontier),
            ..QueryRequirements::default()
        };

        let request = CheckoutQuery::new(task)
            .with_requirements(requirements)
            .compile_checkout()?;

        assert_eq!(request.activation, Some(ActivationState::Frontier));
        Ok(())
    }

    #[test]
    fn checkout_query_compiles_semantic_anchor_requirement(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let anchor = SemanticAnchor::new("project:continuitydb:release-status");
        let task = QueryTask::new("release-status", "what is the release status?");
        let requirements = QueryRequirements {
            semantic_anchor: Some(anchor.clone()),
            ..QueryRequirements::default()
        };

        let request = CheckoutQuery::new(task)
            .with_requirements(requirements)
            .compile_checkout()?;

        assert_eq!(request.semantic_anchor, Some(anchor));
        Ok(())
    }

    #[test]
    fn checkout_query_compiles_context_profile_requirement(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let task = QueryTask::new("failure-review", "why did release upload fail?");
        let requirements = QueryRequirements {
            context_profile: ContextProfile::Debugging,
            ..QueryRequirements::default()
        };

        let request = CheckoutQuery::new(task)
            .with_requirements(requirements)
            .compile_checkout()?;

        assert_eq!(request.context_profile, ContextProfile::Debugging);
        Ok(())
    }

    #[test]
    fn checkout_query_compiles_compiler_policy_and_intent_separate_from_answerability(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let request = CheckoutQuery::new(QueryTask::new(
            "artifact-state",
            "what is the artifact state?",
        ))
        .with_compiler_policy(ContextCompilerPolicy::Automatic)
        .with_compiler_intent("what should I do next without repeating the release upload failure?")
        .compile_checkout()?;

        assert_eq!(
            request.answerability_question.as_deref(),
            Some("what is the artifact state?")
        );
        assert_eq!(
            request.compiler_intent.as_deref(),
            Some("what should I do next without repeating the release upload failure?")
        );
        assert_eq!(request.compiler_policy, ContextCompilerPolicy::Automatic);
        Ok(())
    }

    #[test]
    fn checkout_query_compiles_model_assisted_compiler_proposals(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let target_cell_id = StateCellId::from_u128(42);
        let proposal = ContextCompilerProposal::new(
            target_cell_id,
            ContextPacketStrategy::FalsificationBrief,
            ContextAbstractionLevel::Falsification,
            vec!["model-assisted-falsification".to_string()],
            vec!["artifact://compiler/proposal/query".to_string()],
        )?;

        let request = CheckoutQuery::new(QueryTask::new(
            "artifact-state",
            "what is the artifact state?",
        ))
        .with_compiler_policy(ContextCompilerPolicy::ModelAssisted)
        .with_compiler_proposals(vec![proposal.clone()])
        .compile_checkout()?;

        assert_eq!(
            request.compiler_policy,
            ContextCompilerPolicy::ModelAssisted
        );
        assert_eq!(request.compiler_proposals, vec![proposal]);
        Ok(())
    }

    #[test]
    fn checkout_query_compiles_lifecycle_stage_requirement(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let task = QueryTask::new("runtime-context", "what should the agent use now?");
        let requirements = QueryRequirements {
            lifecycle_stage: Some(LifecycleStage::Operationalized),
            projection_kind: None,
            minimum_uncertainty: None,
            minimum_surprise_bits: None,
            minimum_salience: None,
            minimum_context_affordance: None,
            minimum_epistemic_pressure: None,
            ..QueryRequirements::default()
        };

        let request = CheckoutQuery::new(task)
            .with_requirements(requirements)
            .compile_checkout()?;

        assert_eq!(
            request.lifecycle_stage,
            Some(LifecycleStage::Operationalized)
        );
        Ok(())
    }

    #[test]
    fn checkout_query_compiles_lifecycle_policy_requirements(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let task = QueryTask::new("safe-use-context", "what must be verified before use?");
        let requirements = QueryRequirements {
            retention_policy: Some(RetentionPolicy::DecayUnlessReinforced),
            use_policy: Some(UsePolicy::VerifyBeforeUse),
            promotion_policy: Some(PromotionPolicy::Manual),
            ..QueryRequirements::default()
        };

        let request = CheckoutQuery::new(task)
            .with_requirements(requirements)
            .compile_checkout()?;

        assert_eq!(
            request.retention_policy,
            Some(RetentionPolicy::DecayUnlessReinforced)
        );
        assert_eq!(request.use_policy, Some(UsePolicy::VerifyBeforeUse));
        assert_eq!(request.promotion_policy, Some(PromotionPolicy::Manual));
        Ok(())
    }

    #[test]
    fn checkout_query_compiles_projection_kind_requirement(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let task = QueryTask::new(
            "procedure-context",
            "what procedure should the agent follow?",
        );
        let requirements = QueryRequirements {
            projection_kind: Some(MemoryProjectionKind::Procedural),
            minimum_uncertainty: None,
            minimum_surprise_bits: None,
            minimum_salience: None,
            minimum_context_affordance: None,
            minimum_epistemic_pressure: None,
            ..QueryRequirements::default()
        };

        let request = CheckoutQuery::new(task)
            .with_requirements(requirements)
            .compile_checkout()?;

        assert_eq!(
            request.projection_kind,
            Some(MemoryProjectionKind::Procedural)
        );
        Ok(())
    }

    #[test]
    fn checkout_query_compiles_uncertainty_requirements() -> Result<(), Box<dyn std::error::Error>>
    {
        let task = QueryTask::new("uncertainty-review", "what uncertain facts need attention?");
        let requirements = QueryRequirements {
            minimum_uncertainty: Some(Confidence::new(0.7)?),
            minimum_surprise_bits: Some(3.0),
            minimum_probability_delta: Some(0.7),
            minimum_salience: Some(0.6),
            minimum_context_affordance: Some(0.7),
            minimum_epistemic_pressure: Some(0.55),
            ..QueryRequirements::default()
        };

        let request = CheckoutQuery::new(task)
            .with_requirements(requirements)
            .compile_checkout()?;

        assert_eq!(request.minimum_uncertainty, Some(Confidence::new(0.7)?));
        assert_eq!(request.minimum_surprise_bits, Some(3.0));
        assert_eq!(request.minimum_probability_delta, Some(0.7));
        assert_eq!(request.minimum_salience, Some(0.6));
        assert_eq!(request.minimum_context_affordance, Some(0.7));
        assert_eq!(request.minimum_epistemic_pressure, Some(0.55));
        Ok(())
    }

    #[test]
    fn checkout_query_compiles_epistemic_action_requirement(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let task = QueryTask::new("verification", "what needs missing evidence?");
        let requirements = QueryRequirements {
            epistemic_action: Some(EpistemicAction::Scavenge),
            epistemic_action_reason: None,
            ..QueryRequirements::default()
        };

        let request = CheckoutQuery::new(task)
            .with_requirements(requirements)
            .compile_checkout()?;

        assert_eq!(request.epistemic_action, Some(EpistemicAction::Scavenge));
        Ok(())
    }

    #[test]
    fn checkout_query_compiles_epistemic_action_reason_requirement(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let task = QueryTask::new("surprise", "what contradicted the baseline?");
        let requirements = QueryRequirements {
            epistemic_action_reason: Some(EpistemicActionReason::HighSurprise),
            ..QueryRequirements::default()
        };

        let request = CheckoutQuery::new(task)
            .with_requirements(requirements)
            .compile_checkout()?;

        assert_eq!(
            request.epistemic_action_reason,
            Some(EpistemicActionReason::HighSurprise)
        );
        Ok(())
    }

    #[test]
    fn checkout_query_compiles_selection_reason_requirement(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let task = QueryTask::new("answerability", "what question is this context for?");
        let requirements = QueryRequirements {
            selection_reason: Some(ContextPacketSelectionReason::Answerability),
            ..QueryRequirements::default()
        };

        let request = CheckoutQuery::new(task)
            .with_requirements(requirements)
            .compile_checkout()?;

        assert_eq!(
            request.selection_reason,
            Some(ContextPacketSelectionReason::Answerability)
        );
        Ok(())
    }

    #[test]
    fn checkout_query_compiles_trajectory_memory_reuse_requirements(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let task = QueryTask::new("trajectory-reuse", "what rollout lesson should be reused?");
        let requirements = QueryRequirements {
            selection_reason: Some(ContextPacketSelectionReason::TrajectoryMemory),
            trajectory_memory_strategy: Some(ContextPacketStrategy::FalsificationBrief),
            minimum_trajectory_memory_confidence: Some(Confidence::new(0.8)?),
            ..QueryRequirements::default()
        };

        let request = CheckoutQuery::new(task)
            .with_requirements(requirements)
            .compile_checkout()?;

        assert_eq!(
            request.selection_reason,
            Some(ContextPacketSelectionReason::TrajectoryMemory)
        );
        assert_eq!(
            request.trajectory_memory_strategy,
            Some(ContextPacketStrategy::FalsificationBrief)
        );
        assert_eq!(
            request.minimum_trajectory_memory_confidence,
            Some(Confidence::new(0.8)?)
        );
        Ok(())
    }

    #[test]
    fn checkout_query_compiles_context_gap_requirements() -> Result<(), Box<dyn std::error::Error>>
    {
        let task = QueryTask::new("context-gap", "what missing context should be scavenged?");
        let requirements = QueryRequirements {
            context_gap_kind: Some(ContextGapKind::MissingEvidence),
            minimum_context_gap_priority: Some(Confidence::new(0.7)?),
            invalidation_condition_kind: None,
            minimum_invalidation_priority: None,
            ..QueryRequirements::default()
        };

        let request = CheckoutQuery::new(task)
            .with_requirements(requirements)
            .compile_checkout()?;

        assert_eq!(
            request.context_gap_kind,
            Some(ContextGapKind::MissingEvidence)
        );
        assert_eq!(
            request.minimum_context_gap_priority,
            Some(Confidence::new(0.7)?)
        );
        Ok(())
    }

    #[test]
    fn checkout_query_compiles_invalidation_condition_requirements(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let task = QueryTask::new(
            "invalidation-condition",
            "what falsification condition should govern this context?",
        );
        let requirements = QueryRequirements {
            invalidation_condition_kind: Some(InvalidationConditionKind::DependencyInvalidated),
            minimum_invalidation_priority: Some(Confidence::new(0.7)?),
            ..QueryRequirements::default()
        };

        let request = CheckoutQuery::new(task)
            .with_requirements(requirements)
            .compile_checkout()?;

        assert_eq!(
            request.invalidation_condition_kind,
            Some(InvalidationConditionKind::DependencyInvalidated)
        );
        assert_eq!(
            request.minimum_invalidation_priority,
            Some(Confidence::new(0.7)?)
        );
        Ok(())
    }

    #[test]
    fn continuity_query_delegates_checkout_compilation() -> Result<(), Box<dyn std::error::Error>> {
        let query = ContinuityQuery::Checkout(CheckoutQuery::new(QueryTask::new(
            "release-readiness",
            "what is the release status?",
        )));

        let request = query.compile_checkout()?;

        assert_eq!(
            request.answerability_question.as_deref(),
            Some("what is the release status?")
        );
        Ok(())
    }

    #[test]
    fn cells_only_return_shape_compiles_checkout_request() -> Result<(), Box<dyn std::error::Error>>
    {
        let request = CheckoutQuery::new(QueryTask::new(
            "release-readiness",
            "what is the release status?",
        ))
        .with_return_shape(QueryReturnShape::CellsOnly)
        .compile_checkout()?;

        assert_eq!(
            request.answerability_question.as_deref(),
            Some("what is the release status?")
        );
        Ok(())
    }

    #[test]
    fn context_packets_only_return_shape_compiles_checkout_request(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let request = CheckoutQuery::new(QueryTask::new(
            "release-readiness",
            "what is the release status?",
        ))
        .with_return_shape(QueryReturnShape::ContextPacketsOnly)
        .compile_checkout()?;

        assert_eq!(
            request.answerability_question.as_deref(),
            Some("what is the release status?")
        );
        assert_eq!(request.context_profile, ContextProfile::Execution);
        Ok(())
    }

    #[test]
    fn summary_only_return_shape_compiles_checkout_request(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let request = CheckoutQuery::new(QueryTask::new(
            "release-readiness",
            "what is the release status?",
        ))
        .with_return_shape(QueryReturnShape::SummaryOnly)
        .compile_checkout()?;

        assert_eq!(
            request.answerability_question.as_deref(),
            Some("what is the release status?")
        );
        Ok(())
    }

    #[test]
    fn unsupported_optimization_fails_with_typed_error() -> Result<(), Box<dyn std::error::Error>> {
        let result = CheckoutQuery::new(QueryTask::new(
            "release-readiness",
            "what is the release status?",
        ))
        .with_optimization(QueryOptimization::TokenCostOnly)
        .compile_checkout();

        let error = result
            .err()
            .ok_or_else(|| std::io::Error::other("unsupported optimization should fail"))?;
        assert_eq!(
            error,
            QueryError::UnsupportedOptimization(QueryOptimization::TokenCostOnly)
        );
        Ok(())
    }

    #[test]
    fn checkout_query_round_trips_json_and_compiles() -> Result<(), Box<dyn std::error::Error>> {
        let valid_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 10, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let task = QueryTask::new("release-readiness", "what is the release status?");
        let query = CheckoutQuery::new(task).with_requirements(QueryRequirements {
            scope: Some(Scope::Project("continuitydb".to_string())),
            valid_at: Some(valid_at),
            minimum_confidence: Confidence::new(0.7)?,
            token_budget: 1200,
            ..QueryRequirements::default()
        });

        let encoded = serde_json::to_vec(&query)?;
        let decoded: CheckoutQuery = serde_json::from_slice(&encoded)?;
        let request = decoded.compile_checkout()?;

        assert_eq!(
            request.scope,
            Some(Scope::Project("continuitydb".to_string()))
        );
        assert_eq!(request.valid_at, Some(valid_at));
        assert_eq!(request.minimum_confidence, Confidence::new(0.7)?);
        assert_eq!(request.token_budget, 1200);
        Ok(())
    }

    #[test]
    fn continuity_query_serializes_with_snake_case_checkout_tag(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let query = ContinuityQuery::Checkout(CheckoutQuery::new(QueryTask::new(
            "release-readiness",
            "what is the release status?",
        )));

        let value = serde_json::to_value(&query)?;
        let decoded: ContinuityQuery = serde_json::from_value(value.clone())?;

        assert!(value.get("checkout").is_some());
        assert_eq!(decoded, query);
        Ok(())
    }

    #[test]
    fn query_semantics_survive_json_round_trip() -> Result<(), Box<dyn std::error::Error>> {
        let query = CheckoutQuery::new(QueryTask::new(
            "release-readiness",
            "what is the release status?",
        ))
        .with_return_shape(QueryReturnShape::CellsOnly)
        .with_optimization(QueryOptimization::TokenCostOnly);

        let value = serde_json::to_value(&query)?;
        assert_eq!(value["return_shape"].as_str(), Some("cells_only"));
        assert_eq!(value["optimization"].as_str(), Some("token_cost_only"));

        let decoded: CheckoutQuery = serde_json::from_value(value)?;
        let error = decoded
            .compile_checkout()
            .err()
            .ok_or_else(|| std::io::Error::other("unsupported optimization should fail"))?;

        assert_eq!(
            error,
            QueryError::UnsupportedOptimization(QueryOptimization::TokenCostOnly)
        );
        Ok(())
    }

    #[test]
    fn checkout_query_deserializes_missing_compiler_fields_as_automatic_defaults(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let query = CheckoutQuery::new(QueryTask::new(
            "release-readiness",
            "what is the release status?",
        ))
        .with_compiler_policy(ContextCompilerPolicy::Automatic)
        .with_compiler_intent("what should I do next?");
        let mut value = serde_json::to_value(query)?;
        let object = value
            .as_object_mut()
            .ok_or_else(|| std::io::Error::other("checkout query did not encode as object"))?;
        object.remove("compiler_policy");
        object.remove("compiler_intent");
        object.remove("compiler_proposals");

        let decoded: CheckoutQuery = serde_json::from_value(value)?;
        let request = decoded.compile_checkout()?;

        assert_eq!(request.compiler_policy, ContextCompilerPolicy::Automatic);
        assert_eq!(request.compiler_intent, None);
        assert!(request.compiler_proposals.is_empty());
        Ok(())
    }

    #[test]
    fn checkout_query_round_trips_model_assisted_compiler_proposals(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let proposal = ContextCompilerProposal::new(
            StateCellId::from_u128(7),
            ContextPacketStrategy::ScavengingBrief,
            ContextAbstractionLevel::Scavenging,
            vec!["model-assisted-scavenging".to_string()],
            vec!["artifact://compiler/proposal/round-trip".to_string()],
        )?;
        let query = CheckoutQuery::new(QueryTask::new(
            "missing-evidence",
            "what evidence should be gathered next?",
        ))
        .with_compiler_policy(ContextCompilerPolicy::ModelAssisted)
        .with_compiler_proposals(vec![proposal.clone()]);

        let decoded: CheckoutQuery = serde_json::from_value(serde_json::to_value(query)?)?;
        let request = decoded.compile_checkout()?;

        assert_eq!(
            request.compiler_policy,
            ContextCompilerPolicy::ModelAssisted
        );
        assert_eq!(request.compiler_proposals, vec![proposal]);
        Ok(())
    }

    #[test]
    fn query_envelope_encodes_format_version_and_query() -> Result<(), Box<dyn std::error::Error>> {
        let query = ContinuityQuery::Checkout(CheckoutQuery::new(QueryTask::new(
            "stored-facts",
            "what is stored?",
        )));

        let encoded = encode_query_json(query.clone())?;
        let value: serde_json::Value = serde_json::from_slice(&encoded)?;

        assert_eq!(value["format"].as_str(), Some(QUERY_ENVELOPE_FORMAT));
        assert_eq!(
            value["version"].as_u64(),
            Some(QUERY_ENVELOPE_FORMAT_VERSION as u64)
        );
        assert!(value["query"]["checkout"].is_object());
        assert_eq!(decode_query_json(&encoded)?, query);
        Ok(())
    }

    #[test]
    fn query_envelope_rejects_unsupported_format() -> Result<(), Box<dyn std::error::Error>> {
        let envelope = QueryEnvelope {
            format: "continuitydb.other".to_string(),
            version: QUERY_ENVELOPE_FORMAT_VERSION,
            query: ContinuityQuery::Checkout(CheckoutQuery::new(QueryTask::new(
                "stored-facts",
                "what is stored?",
            ))),
        };
        let encoded = serde_json::to_vec(&envelope)?;

        assert_eq!(
            decode_query_json(&encoded),
            Err(QueryEnvelopeError::InvalidEnvelope)
        );
        Ok(())
    }

    #[test]
    fn query_envelope_rejects_unsupported_version() -> Result<(), Box<dyn std::error::Error>> {
        let envelope = QueryEnvelope {
            format: QUERY_ENVELOPE_FORMAT.to_string(),
            version: QUERY_ENVELOPE_FORMAT_VERSION + 1,
            query: ContinuityQuery::Checkout(CheckoutQuery::new(QueryTask::new(
                "stored-facts",
                "what is stored?",
            ))),
        };
        let encoded = serde_json::to_vec(&envelope)?;

        assert_eq!(
            decode_query_json(&encoded),
            Err(QueryEnvelopeError::InvalidEnvelope)
        );
        Ok(())
    }

    #[test]
    fn query_envelope_rejects_malformed_json() {
        assert_eq!(
            decode_query_json(b"{not valid json}\n"),
            Err(QueryEnvelopeError::InvalidJson)
        );
    }

    #[test]
    fn checkout_query_accessors_expose_typed_semantics() -> Result<(), Box<dyn std::error::Error>> {
        let valid_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 10, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let requirements = QueryRequirements {
            scope: Some(Scope::Project("continuitydb".to_string())),
            valid_at: Some(valid_at),
            minimum_confidence: Confidence::new(0.7)?,
            token_budget: 1200,
            ..QueryRequirements::default()
        };
        let query = CheckoutQuery::new(QueryTask::new("stored-facts", "what is stored?"))
            .with_requirements(requirements.clone())
            .with_return_shape(QueryReturnShape::CellsOnly)
            .with_optimization(QueryOptimization::TokenCostOnly);

        assert_eq!(query.task().name, "stored-facts");
        assert_eq!(query.task().answerability_question, "what is stored?");
        assert_eq!(query.requirements(), &requirements);
        assert_eq!(query.return_shape(), QueryReturnShape::CellsOnly);
        assert_eq!(query.optimization(), QueryOptimization::TokenCostOnly);
        Ok(())
    }

    #[test]
    fn decoded_query_envelope_can_be_inspected_before_compile(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let query = ContinuityQuery::Checkout(
            CheckoutQuery::new(QueryTask::new("stored-facts", "what is stored?"))
                .with_return_shape(QueryReturnShape::CellsOnly),
        );
        let encoded = encode_query_json(query)?;
        let decoded = decode_query_json(&encoded)?;

        let ContinuityQuery::Checkout(checkout) = decoded;
        assert_eq!(checkout.task().name, "stored-facts");
        assert_eq!(checkout.return_shape(), QueryReturnShape::CellsOnly);
        assert_eq!(
            checkout
                .compile_checkout()?
                .answerability_question
                .as_deref(),
            Some("what is stored?")
        );
        Ok(())
    }

    #[test]
    fn text_query_parses_minimal_checkout() -> Result<(), Box<dyn std::error::Error>> {
        let query = parse_query_text(r#"CHECKOUT "release" ANSWER "what should ship?""#)?;

        let ContinuityQuery::Checkout(checkout) = query;
        assert_eq!(checkout.task().name, "release");
        assert_eq!(checkout.task().answerability_question, "what should ship?");
        assert_eq!(checkout.requirements(), &QueryRequirements::default());
        Ok(())
    }

    #[test]
    fn text_query_parses_checkout_where_constraints() -> Result<(), Box<dyn std::error::Error>> {
        let query = parse_query_text(
            r#"CHECKOUT "release" ANSWER "what should ship?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
  AND token_budget <= 1200
  AND evidence_source = "source:release-notes""#,
        )?;

        let ContinuityQuery::Checkout(checkout) = query;
        assert_eq!(
            checkout.requirements().scope,
            Some(Scope::Project("continuitydb".to_string()))
        );
        assert_eq!(
            checkout.requirements().minimum_confidence,
            Confidence::new(0.7)?
        );
        assert_eq!(checkout.requirements().token_budget, 1200);
        assert_eq!(
            checkout.requirements().evidence_source.as_deref(),
            Some("source:release-notes")
        );
        Ok(())
    }

    #[test]
    fn text_query_parses_temporal_and_commit_constraints() -> Result<(), Box<dyn std::error::Error>>
    {
        let commit_id = CommitId::new();
        let query = parse_query_text(&format!(
            r#"CHECKOUT "release" ANSWER "what should ship?"
WHERE valid_at = "2026-05-20T12:00:00Z"
  AND system_at = "2026-05-20T12:30:00Z"
  AND commit_id = "{commit_id}""#
        ))?;

        let ContinuityQuery::Checkout(checkout) = query;
        assert_eq!(
            checkout.requirements().valid_at,
            Some(
                Utc.with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
                    .single()
                    .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?
            )
        );
        assert_eq!(
            checkout.requirements().system_at,
            Some(
                Utc.with_ymd_and_hms(2026, 5, 20, 12, 30, 0)
                    .single()
                    .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?
            )
        );
        assert_eq!(checkout.requirements().commit_id, Some(commit_id));
        Ok(())
    }

    #[test]
    fn text_query_parses_dependency_constraints() -> Result<(), Box<dyn std::error::Error>> {
        let dependency_target = StateCellId::new();
        let query = parse_query_text(&format!(
            r#"CHECKOUT "release" ANSWER "what should ship?"
WHERE dependency_target = "{dependency_target}"
  AND dependency_kind = derived_from"#
        ))?;

        let ContinuityQuery::Checkout(checkout) = query;
        assert_eq!(
            checkout.requirements().dependency_target,
            Some(dependency_target)
        );
        assert_eq!(
            checkout.requirements().dependency_kind,
            Some(CellDependencyKind::DerivedFrom)
        );
        Ok(())
    }

    #[test]
    fn text_query_parses_revision_link_constraints() -> Result<(), Box<dyn std::error::Error>> {
        let related = "00000000-0000-0000-0000-00000000002a";
        let query = parse_query_text(&format!(
            r#"CHECKOUT "conflict-review" ANSWER "what conflicts with this cell?"
WHERE revision_related_cell = "{related}" AND revision_link_kind = conflicts_with"#
        ))?;

        let ContinuityQuery::Checkout(checkout) = query;
        assert_eq!(
            checkout.requirements().revision_related_cell,
            Some(related.parse::<StateCellId>()?)
        );
        assert_eq!(
            checkout.requirements().revision_link_kind,
            Some(RevisionLinkKind::ConflictsWith)
        );
        Ok(())
    }

    #[test]
    fn text_query_parses_summary_only_return_shape() -> Result<(), Box<dyn std::error::Error>> {
        let query = parse_query_text(
            r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
RETURN summary_only"#,
        )?;

        let ContinuityQuery::Checkout(checkout) = query;
        assert_eq!(checkout.return_shape(), QueryReturnShape::SummaryOnly);
        assert_eq!(
            checkout.requirements().scope,
            Some(Scope::Project("continuitydb".to_string()))
        );
        Ok(())
    }

    #[test]
    fn text_query_parses_cells_only_return_shape() -> Result<(), Box<dyn std::error::Error>> {
        let query = parse_query_text(
            r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
RETURN cells_only"#,
        )?;

        let ContinuityQuery::Checkout(checkout) = query;
        assert_eq!(checkout.return_shape(), QueryReturnShape::CellsOnly);
        assert_eq!(
            checkout.requirements().scope,
            Some(Scope::Project("continuitydb".to_string()))
        );
        Ok(())
    }

    #[test]
    fn text_query_parses_context_packets_only_return_shape(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let query = parse_query_text(
            r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
RETURN context_packets_only"#,
        )?;

        let ContinuityQuery::Checkout(checkout) = query;
        assert_eq!(
            checkout.return_shape(),
            QueryReturnShape::ContextPacketsOnly
        );
        assert_eq!(
            checkout.requirements().scope,
            Some(Scope::Project("continuitydb".to_string()))
        );
        Ok(())
    }

    #[test]
    fn text_query_parses_activation_constraint() -> Result<(), Box<dyn std::error::Error>> {
        let query = parse_query_text(
            r#"CHECKOUT "frontier-review" ANSWER "what frontier cells need review?"
WHERE activation = frontier"#,
        )?;

        let ContinuityQuery::Checkout(checkout) = query;
        assert_eq!(
            checkout.requirements().activation,
            Some(ActivationState::Frontier)
        );
        Ok(())
    }

    #[test]
    fn text_query_parses_context_profile_constraint() -> Result<(), Box<dyn std::error::Error>> {
        let query = parse_query_text(
            r#"CHECKOUT "failure-review" ANSWER "why did release upload fail?"
WHERE context_profile = debugging"#,
        )?;

        let ContinuityQuery::Checkout(checkout) = query;
        assert_eq!(
            checkout.requirements().context_profile,
            ContextProfile::Debugging
        );
        assert_eq!(
            checkout.compile_checkout()?.context_profile,
            ContextProfile::Debugging
        );
        Ok(())
    }

    #[test]
    fn text_query_parses_compiler_policy_and_intent() -> Result<(), Box<dyn std::error::Error>> {
        let query = parse_query_text(
            r#"CHECKOUT "artifact-state" ANSWER "what is the artifact state?"
WHERE compiler_policy = automatic
  AND compiler_intent = "what should I do next without repeating the release upload failure?""#,
        )?;

        let ContinuityQuery::Checkout(checkout) = query;
        assert_eq!(checkout.compiler_policy(), ContextCompilerPolicy::Automatic);
        assert_eq!(
            checkout.compiler_intent(),
            Some("what should I do next without repeating the release upload failure?")
        );
        let request = checkout.compile_checkout()?;
        assert_eq!(
            request.answerability_question.as_deref(),
            Some("what is the artifact state?")
        );
        assert_eq!(
            request.compiler_intent.as_deref(),
            Some("what should I do next without repeating the release upload failure?")
        );
        assert_eq!(request.compiler_policy, ContextCompilerPolicy::Automatic);
        Ok(())
    }

    #[test]
    fn text_query_parses_lifecycle_stage_constraint() -> Result<(), Box<dyn std::error::Error>> {
        let query = parse_query_text(
            r#"CHECKOUT "runtime-context" ANSWER "what should the agent use now?"
WHERE lifecycle_stage = operationalized"#,
        )?;

        let ContinuityQuery::Checkout(checkout) = query;
        assert_eq!(
            checkout.requirements().lifecycle_stage,
            Some(LifecycleStage::Operationalized)
        );
        assert_eq!(
            checkout.compile_checkout()?.lifecycle_stage,
            Some(LifecycleStage::Operationalized)
        );
        Ok(())
    }

    #[test]
    fn text_query_parses_lifecycle_policy_constraints() -> Result<(), Box<dyn std::error::Error>> {
        let query = parse_query_text(
            r#"CHECKOUT "safe-use-context" ANSWER "what must be verified before use?"
WHERE retention_policy = decay_unless_reinforced AND use_policy = verify_before_use AND promotion_policy = manual"#,
        )?;

        let ContinuityQuery::Checkout(checkout) = query;
        assert_eq!(
            checkout.requirements().retention_policy,
            Some(RetentionPolicy::DecayUnlessReinforced)
        );
        assert_eq!(
            checkout.requirements().use_policy,
            Some(UsePolicy::VerifyBeforeUse)
        );
        assert_eq!(
            checkout.requirements().promotion_policy,
            Some(PromotionPolicy::Manual)
        );
        let request = checkout.compile_checkout()?;
        assert_eq!(request.use_policy, Some(UsePolicy::VerifyBeforeUse));
        Ok(())
    }

    #[test]
    fn text_query_parses_projection_kind_constraint() -> Result<(), Box<dyn std::error::Error>> {
        let query = parse_query_text(
            r#"CHECKOUT "procedure-context" ANSWER "what procedure should the agent follow?"
WHERE projection_kind = procedural"#,
        )?;

        let ContinuityQuery::Checkout(checkout) = query;
        assert_eq!(
            checkout.requirements().projection_kind,
            Some(MemoryProjectionKind::Procedural)
        );
        assert_eq!(
            checkout.compile_checkout()?.projection_kind,
            Some(MemoryProjectionKind::Procedural)
        );
        Ok(())
    }

    #[test]
    fn text_query_parses_uncertainty_constraints() -> Result<(), Box<dyn std::error::Error>> {
        let query = parse_query_text(
            r#"CHECKOUT "uncertainty-review" ANSWER "what uncertain facts need attention?"
WHERE min_uncertainty >= 0.7 AND min_surprise_bits >= 3.0 AND min_probability_delta >= 0.7 AND min_salience >= 0.6 AND min_context_affordance >= 0.7 AND min_epistemic_pressure >= 0.55"#,
        )?;

        let ContinuityQuery::Checkout(checkout) = query;
        assert_eq!(
            checkout.requirements().minimum_uncertainty,
            Some(Confidence::new(0.7)?)
        );
        assert_eq!(checkout.requirements().minimum_surprise_bits, Some(3.0));
        assert_eq!(checkout.requirements().minimum_probability_delta, Some(0.7));
        assert_eq!(checkout.requirements().minimum_salience, Some(0.6));
        assert_eq!(
            checkout.requirements().minimum_context_affordance,
            Some(0.7)
        );
        assert_eq!(
            checkout.requirements().minimum_epistemic_pressure,
            Some(0.55)
        );
        let request = checkout.compile_checkout()?;
        assert_eq!(request.minimum_uncertainty, Some(Confidence::new(0.7)?));
        assert_eq!(request.minimum_surprise_bits, Some(3.0));
        assert_eq!(request.minimum_probability_delta, Some(0.7));
        assert_eq!(request.minimum_salience, Some(0.6));
        assert_eq!(request.minimum_context_affordance, Some(0.7));
        assert_eq!(request.minimum_epistemic_pressure, Some(0.55));
        Ok(())
    }

    #[test]
    fn text_query_parses_epistemic_action_constraint() -> Result<(), Box<dyn std::error::Error>> {
        let query = parse_query_text(
            r#"CHECKOUT "verification" ANSWER "what needs missing evidence?"
WHERE epistemic_action = scavenge"#,
        )?;

        let ContinuityQuery::Checkout(checkout) = query;
        assert_eq!(
            checkout.requirements().epistemic_action,
            Some(EpistemicAction::Scavenge)
        );
        assert_eq!(
            checkout.compile_checkout()?.epistemic_action,
            Some(EpistemicAction::Scavenge)
        );
        Ok(())
    }

    #[test]
    fn text_query_parses_epistemic_action_reason_constraint(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let query = parse_query_text(
            r#"CHECKOUT "surprise" ANSWER "what contradicted the baseline?"
WHERE epistemic_action_reason = high_surprise"#,
        )?;

        let ContinuityQuery::Checkout(checkout) = query;
        assert_eq!(
            checkout.requirements().epistemic_action_reason,
            Some(EpistemicActionReason::HighSurprise)
        );
        assert_eq!(
            checkout.compile_checkout()?.epistemic_action_reason,
            Some(EpistemicActionReason::HighSurprise)
        );
        Ok(())
    }

    #[test]
    fn text_query_parses_selection_reason_constraint() -> Result<(), Box<dyn std::error::Error>> {
        let query = parse_query_text(
            r#"CHECKOUT "context-gap" ANSWER "what missing context should be scavenged?"
WHERE selection_reason = context_gap"#,
        )?;

        let ContinuityQuery::Checkout(checkout) = query;
        assert_eq!(
            checkout.requirements().selection_reason,
            Some(ContextPacketSelectionReason::ContextGap)
        );
        assert_eq!(
            checkout.compile_checkout()?.selection_reason,
            Some(ContextPacketSelectionReason::ContextGap)
        );
        Ok(())
    }

    #[test]
    fn text_query_parses_trajectory_memory_reuse_constraints(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let query = parse_query_text(
            r#"CHECKOUT "trajectory-reuse" ANSWER "what rollout lesson should be reused?"
WHERE selection_reason = trajectory_memory
  AND trajectory_memory_strategy = falsification_brief
  AND min_trajectory_memory_confidence >= 0.8"#,
        )?;

        let ContinuityQuery::Checkout(checkout) = query;
        assert_eq!(
            checkout.requirements().selection_reason,
            Some(ContextPacketSelectionReason::TrajectoryMemory)
        );
        assert_eq!(
            checkout.requirements().trajectory_memory_strategy,
            Some(ContextPacketStrategy::FalsificationBrief)
        );
        assert_eq!(
            checkout.requirements().minimum_trajectory_memory_confidence,
            Some(Confidence::new(0.8)?)
        );
        let request = checkout.compile_checkout()?;
        assert_eq!(
            request.trajectory_memory_strategy,
            Some(ContextPacketStrategy::FalsificationBrief)
        );
        assert_eq!(
            request.minimum_trajectory_memory_confidence,
            Some(Confidence::new(0.8)?)
        );
        Ok(())
    }

    #[test]
    fn text_query_parses_context_gap_constraints() -> Result<(), Box<dyn std::error::Error>> {
        let query = parse_query_text(
            r#"CHECKOUT "context-gap" ANSWER "what missing context should be scavenged?"
WHERE context_gap_kind = missing_evidence AND min_context_gap_priority >= 0.7"#,
        )?;

        let ContinuityQuery::Checkout(checkout) = query;
        assert_eq!(
            checkout.requirements().context_gap_kind,
            Some(ContextGapKind::MissingEvidence)
        );
        assert_eq!(
            checkout.requirements().minimum_context_gap_priority,
            Some(Confidence::new(0.7)?)
        );
        let request = checkout.compile_checkout()?;
        assert_eq!(
            request.context_gap_kind,
            Some(ContextGapKind::MissingEvidence)
        );
        assert_eq!(
            request.minimum_context_gap_priority,
            Some(Confidence::new(0.7)?)
        );
        Ok(())
    }

    #[test]
    fn text_query_parses_invalidation_condition_constraints(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let query = parse_query_text(
            r#"CHECKOUT "invalidation-condition" ANSWER "what falsifies this belief?"
WHERE invalidation_condition_kind = dependency_invalidated AND min_invalidation_priority >= 0.7"#,
        )?;

        let ContinuityQuery::Checkout(checkout) = query;
        assert_eq!(
            checkout.requirements().invalidation_condition_kind,
            Some(InvalidationConditionKind::DependencyInvalidated)
        );
        assert_eq!(
            checkout.requirements().minimum_invalidation_priority,
            Some(Confidence::new(0.7)?)
        );
        let request = checkout.compile_checkout()?;
        assert_eq!(
            request.invalidation_condition_kind,
            Some(InvalidationConditionKind::DependencyInvalidated)
        );
        assert_eq!(
            request.minimum_invalidation_priority,
            Some(Confidence::new(0.7)?)
        );
        Ok(())
    }

    #[test]
    fn text_query_parses_semantic_anchor_constraint() -> Result<(), Box<dyn std::error::Error>> {
        let query = parse_query_text(
            r#"CHECKOUT "release-status" ANSWER "what is the release status?"
WHERE semantic_anchor = "project:continuitydb:release-status""#,
        )?;

        let ContinuityQuery::Checkout(checkout) = query;
        assert_eq!(
            checkout.requirements().semantic_anchor,
            Some(SemanticAnchor::new("project:continuitydb:release-status"))
        );
        Ok(())
    }

    #[test]
    fn text_query_keywords_are_case_insensitive() -> Result<(), Box<dyn std::error::Error>> {
        let query = parse_query_text(
            r#"checkout "release" answer "what should ship?" where scope = global"#,
        )?;

        let ContinuityQuery::Checkout(checkout) = query;
        assert_eq!(checkout.requirements().scope, Some(Scope::Global));
        Ok(())
    }

    #[test]
    fn text_query_rejects_invalid_syntax() {
        assert_eq!(
            parse_query_text(r#"CHECKOUT "release" WHERE scope = global"#),
            Err(QueryTextError::InvalidSyntax)
        );
    }

    #[test]
    fn text_query_rejects_invalid_values() {
        assert_eq!(
            parse_query_text(
                r#"CHECKOUT "release" ANSWER "what should ship?" WHERE min_confidence >= 1.5"#
            ),
            Err(QueryTextError::InvalidValue)
        );
        assert_eq!(
            parse_query_text(
                r#"CHECKOUT "release" ANSWER "what should ship?" WHERE token_budget <= -1"#
            ),
            Err(QueryTextError::InvalidValue)
        );
        assert_eq!(
            parse_query_text(
                r#"CHECKOUT "release" ANSWER "what should ship?" WHERE valid_at = "not-a-time""#
            ),
            Err(QueryTextError::InvalidValue)
        );
        assert_eq!(
            parse_query_text(
                r#"CHECKOUT "release" ANSWER "what should ship?" WHERE commit_id = "not-a-uuid""#
            ),
            Err(QueryTextError::InvalidValue)
        );
        assert_eq!(
            parse_query_text(
                r#"CHECKOUT "release" ANSWER "what should ship?" WHERE dependency_target = "not-a-uuid""#
            ),
            Err(QueryTextError::InvalidValue)
        );
        assert_eq!(
            parse_query_text(
                r#"CHECKOUT "release" ANSWER "what should ship?" WHERE dependency_kind = unknown_kind"#
            ),
            Err(QueryTextError::InvalidValue)
        );
        assert_eq!(
            parse_query_text(
                r#"CHECKOUT "release" ANSWER "what should ship?" WHERE activation = unknown"#
            ),
            Err(QueryTextError::InvalidValue)
        );
        assert_eq!(
            parse_query_text(
                r#"CHECKOUT "release" ANSWER "what should ship?" WHERE compiler_policy = arbitrary"#
            ),
            Err(QueryTextError::InvalidValue)
        );
    }
}
