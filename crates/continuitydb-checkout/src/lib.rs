//! Deterministic checkout and audit.

use chrono::{DateTime, Utc};
use continuitydb_core::{
    ActivationState, CellDependencyKind, CommitId, Confidence, ContextAbstractionLevel,
    ContextCompilerPolicy, ContextCompilerProposal, ContextCompilerProposalLine, ContextGapKind,
    ContextLifecycleEvaluation, ContextPacket, ContextPacketDependencyContext, ContextPacketEntry,
    ContextPacketEntrySource, ContextPacketPlan, ContextPacketPurpose, ContextPacketRequirement,
    ContextPacketRevisionContext, ContextPacketRevisionRelation, ContextPacketSelectionReason,
    ContextPacketStrategy, ContextProfile, CoreError, EpistemicAction, EpistemicActionReason,
    EpistemicPressure, InvalidationConditionKind, LifecycleStage, MemoryProjectionKind,
    PromotionPolicy, RetentionPolicy, RevisionLinkKind, RevisionLinkRecord, Scope, SemanticAnchor,
    StateCell, StateCellId, TrajectoryMemory, TrustSignal, UsePolicy,
};
use continuitydb_kernel::{CellLookup, KernelError, RevisionLinkLookup, StorageKernel};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use thiserror::Error;

/// Errors produced by checkout.
#[derive(Debug, Error, PartialEq)]
pub enum CheckoutError {
    /// Storage kernel failure.
    #[error(transparent)]
    Kernel(#[from] KernelError),
    /// Checkout metadata could not be derived from selected cells.
    #[error(transparent)]
    Core(#[from] CoreError),
    /// Model-proposed compilation was requested without an accepted proposal.
    #[error("model-assisted context compilation requires an accepted compiler proposal")]
    MissingContextCompilerProposal,
    /// Model-proposed compilation found more than one accepted proposal for a selected cell.
    #[error("model-assisted context compilation requires exactly one compiler proposal per selected cell")]
    AmbiguousContextCompilerProposal,
}

/// Request constraints for deterministic checkout.
#[derive(Clone, Debug)]
pub struct CheckoutRequest {
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
    /// Optional deterministic packet selection reason filter.
    pub selection_reason: Option<ContextPacketSelectionReason>,
    /// Optional StateCell v2 trajectory-memory checkout strategy filter.
    pub trajectory_memory_strategy: Option<ContextPacketStrategy>,
    /// Optional minimum StateCell v2 trajectory-memory confidence filter.
    pub minimum_trajectory_memory_confidence: Option<Confidence>,
    /// Optional exact answerability question filter.
    pub answerability_question: Option<String>,
    /// Optional task intent used only to shape compiler output.
    pub compiler_intent: Option<String>,
    /// Accepted model-assisted compiler proposals available to shape selected packets.
    pub compiler_proposals: Vec<ContextCompilerProposal>,
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
    /// Compiler policy used to shape selected cells into model-facing packets.
    pub compiler_policy: ContextCompilerPolicy,
    /// Minimum evidence confidence for included cells.
    pub minimum_confidence: Confidence,
    /// Maximum token budget for the returned slice.
    pub token_budget: i64,
}

/// Materialized continuity slice.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CheckoutSlice {
    /// Selected cells.
    pub cells: Vec<StateCell>,
    /// Total estimated tokens.
    pub total_tokens: i64,
    /// Deterministic aggregate metadata for the bounded checkout context.
    pub summary: CheckoutSummary,
    /// Citation traces for selected cells.
    pub audit_traces: Vec<AuditTrace>,
    /// Per-cell uncertainty metadata for selected cells.
    pub uncertainty: Vec<UncertaintyEntry>,
    /// Per-cell projection packets materialized for the requested context profile.
    pub context_packets: Vec<ContextPacket>,
    /// Selected frontier cells that should remain monitored.
    pub frontier_recommendations: Vec<FrontierRecommendation>,
    /// Candidate cells that matched constraints but were not selected.
    pub alternatives: Vec<CheckoutAlternative>,
}

/// Deterministic aggregate metadata for a materialized continuity slice.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CheckoutSummary {
    /// Number of selected cells in the bounded slice.
    pub selected_cell_count: usize,
    /// Number of matching candidates omitted from the bounded slice.
    pub alternative_count: usize,
    /// Total estimated token cost of selected cells.
    pub total_tokens: i64,
    /// Requested maximum token budget.
    pub token_budget: i64,
    /// Number of selected-cell citation locators preserved in audit traces.
    pub citation_count: usize,
    /// Number of uncertainty entries preserved for selected cells.
    pub uncertainty_count: usize,
    /// Number of projection packets materialized for selected cells.
    pub context_packet_count: usize,
    /// Number of selected frontier cells that should remain monitored.
    pub frontier_recommendation_count: usize,
    /// Number of native revision links preserved in selected-cell audit traces.
    pub revision_link_count: usize,
    /// Number of compact revision-neighborhood contexts preserved in audit traces.
    pub revision_context_count: usize,
    /// Ordered counts of selected packets by deterministic epistemic action.
    #[serde(default)]
    pub epistemic_action_counts: Vec<EpistemicActionCount>,
    /// Ordered counts of deterministic reasons behind selected packet actions.
    #[serde(default)]
    pub epistemic_action_reason_counts: Vec<EpistemicActionReasonCount>,
    /// Ordered counts of deterministic reasons selected packets are useful.
    #[serde(default)]
    pub selection_reason_counts: Vec<SelectionReasonCount>,
    /// Maximum derived epistemic pressure signals across selected context packets.
    #[serde(default)]
    pub epistemic_pressure: EpistemicPressureSummary,
    /// Highest selected-packet native value-of-context score.
    #[serde(default)]
    pub maximum_context_affordance_score: f32,
    /// Highest selected-packet native attention salience score.
    #[serde(default)]
    pub maximum_salience_score: f32,
    /// Number of structured missing-context gaps preserved across selected packets.
    #[serde(default)]
    pub context_gap_count: usize,
    /// Ordered counts of selected packet context gaps by native missing-context kind.
    #[serde(default)]
    pub context_gap_kind_counts: Vec<ContextGapKindCount>,
    /// Highest selected-packet missing-context gap priority, if any gaps were selected.
    #[serde(default)]
    pub maximum_context_gap_priority: Option<Confidence>,
    /// Number of structured invalidation conditions preserved across selected packets.
    #[serde(default)]
    pub invalidation_condition_count: usize,
    /// Ordered counts of selected packet invalidation conditions by native falsifier kind.
    #[serde(default)]
    pub invalidation_condition_kind_counts: Vec<InvalidationConditionKindCount>,
    /// Highest selected-packet invalidation-condition priority, if any conditions were selected.
    #[serde(default)]
    pub maximum_invalidation_priority: Option<Confidence>,
    /// Minimum selected-cell evidence confidence, if any cells were selected.
    pub minimum_selected_confidence: Option<Confidence>,
    /// Maximum selected-cell evidence confidence, if any cells were selected.
    pub maximum_selected_confidence: Option<Confidence>,
    /// Whether selected cells fit inside the requested token budget.
    pub bounded_by_token_budget: bool,
}

/// Count of selected packets with a deterministic epistemic action.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EpistemicActionCount {
    /// Epistemic action assigned to selected packet context.
    pub action: EpistemicAction,
    /// Number of selected context packets with this action.
    pub count: usize,
}

/// Count of selected packets with a deterministic epistemic-action reason.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EpistemicActionReasonCount {
    /// Epistemic-action reason assigned to selected packet context.
    pub reason: EpistemicActionReason,
    /// Number of selected context packets carrying this action reason.
    pub count: usize,
}

/// Count of selected packets with a deterministic selection reason.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SelectionReasonCount {
    /// Selection reason assigned to selected packet context.
    pub reason: ContextPacketSelectionReason,
    /// Number of selected context packets carrying this selection reason.
    pub count: usize,
}

/// Count of selected packet missing-context gaps by gap kind.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ContextGapKindCount {
    /// Native missing-context gap kind.
    pub kind: ContextGapKind,
    /// Number of selected context gaps with this kind.
    pub count: usize,
}

/// Count of selected packet invalidation conditions by condition kind.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct InvalidationConditionKindCount {
    /// Native invalidation-condition kind.
    pub kind: InvalidationConditionKind,
    /// Number of selected invalidation conditions with this kind.
    pub count: usize,
}

/// Aggregate derived epistemic pressure across a checkout slice.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EpistemicPressureSummary {
    /// Highest selected-packet revision pressure.
    pub maximum_revision_pressure: f32,
    /// Highest selected-packet scavenging pressure.
    pub maximum_scavenging_pressure: f32,
    /// Highest selected-packet checkout pressure.
    pub maximum_checkout_pressure: f32,
}

/// Deterministic uncertainty metadata for a selected StateCell.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UncertaintyEntry {
    /// Selected cell identifier.
    pub cell_id: StateCellId,
    /// Maximum confidence across the selected cell's evidence.
    pub max_confidence: Confidence,
    /// Native StateCell uncertainty score.
    pub uncertainty_score: Confidence,
    /// Native StateCell surprise bits relative to a prior baseline.
    pub surprise_bits: f32,
    /// Native StateCell uncertainty rationale, when recorded.
    pub rationale: Option<String>,
}

/// Recommendation to keep a selected frontier StateCell under monitoring.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FrontierRecommendation {
    /// Frontier cell identifier.
    pub cell_id: StateCellId,
    /// Citation locators supporting the frontier recommendation.
    pub citations: Vec<String>,
}

/// Deterministic reason a matching candidate was not selected.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CheckoutAlternativeReason {
    /// Candidate could not fit inside the remaining token budget.
    TokenBudgetExceeded,
}

/// Metadata for a matching candidate omitted from the selected slice.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CheckoutAlternative {
    /// Omitted candidate cell identifier.
    pub cell_id: StateCellId,
    /// Deterministic omission reason.
    pub reason: CheckoutAlternativeReason,
    /// Citation locators supporting the omitted candidate.
    pub citations: Vec<String>,
    /// Maximum confidence across the omitted candidate's evidence.
    pub max_confidence: Confidence,
}

/// Audit trace for a StateCell.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AuditTrace {
    /// Audited cell identifier.
    pub cell_id: StateCellId,
    /// Database commit boundary that wrote the audited cell.
    pub commit_id: CommitId,
    /// Citation locators supporting the cell.
    pub citations: Vec<String>,
    /// Structured evidence provenance supporting the cell.
    pub evidence: Vec<AuditEvidence>,
    /// Dependency and causality links referenced by the cell.
    pub dependencies: Vec<AuditDependency>,
    /// Native revision-link records where this cell participates.
    pub revision_links: Vec<RevisionLinkRecord>,
    /// Compact evidence context for cells connected by native revision links.
    pub revision_context: Vec<AuditRevisionContext>,
}

/// Evidence metadata included in an audit trace.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AuditEvidence {
    /// Stable evidence source identifier.
    pub source: String,
    /// Source-local citation URI, path, or durable locator.
    pub locator: String,
    /// Confidence assigned to this evidence.
    pub confidence: Confidence,
    /// Trust signals associated with this evidence.
    pub trust: Vec<TrustSignal>,
}

/// Dependency metadata included in an audit trace.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AuditDependency {
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

/// Compact audit context for a StateCell connected by a revision link.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AuditRevisionContext {
    /// Selected cell whose audit trace contains this context.
    pub selected_cell_id: StateCellId,
    /// Related cell reached through the revision link.
    pub related_cell_id: StateCellId,
    /// Revision relationship from the selected cell's perspective.
    pub relation: AuditRevisionRelation,
    /// Native revision link kind connecting the cells.
    pub kind: continuitydb_core::RevisionLinkKind,
    /// Semantic anchors on the related cell.
    #[serde(default)]
    pub anchors: Vec<SemanticAnchor>,
    /// Citation locators supporting the related cell.
    pub citations: Vec<String>,
    /// Maximum confidence across the related cell's evidence.
    pub max_confidence: Confidence,
    /// Activation state of the related cell.
    pub activation: ActivationState,
}

/// Direction of a revision link relative to the selected cell.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum AuditRevisionRelation {
    /// The selected cell is the source of the revision link.
    SourceToTarget,
    /// The selected cell is the target of the revision link.
    TargetFromSource,
    /// The selected cell appears as both source and target.
    SelfLink,
}

/// Materializes a deterministic continuity slice.
pub fn checkout<K: StorageKernel>(
    kernel: &K,
    request: CheckoutRequest,
) -> Result<CheckoutSlice, CheckoutError> {
    let mut candidates = kernel.lookup_cells(cell_lookup_from_checkout_request(&request))?;

    candidates.retain(|cell| {
        cell.evidence
            .iter()
            .any(|evidence| evidence.confidence.value() >= request.minimum_confidence.value())
            && request
                .epistemic_action
                .map_or(true, |action| cell.epistemic_action() == action)
            && request.epistemic_action_reason.map_or(true, |reason| {
                cell.epistemic_action_reasons().contains(&reason)
            })
            && request.selection_reason.map_or(true, |reason| {
                cell.context_packet(request.context_profile, i64::MAX)
                    .selection
                    .is_some_and(|selection| selection.reasons.contains(&reason))
            })
    });
    let mut revision_filtered = Vec::new();
    for cell in candidates {
        if cell_matches_revision_constraints(
            kernel,
            cell.id,
            request.revision_related_cell,
            request.revision_link_kind,
        )? {
            revision_filtered.push(cell);
        }
    }
    let mut candidates = revision_filtered;

    candidates.sort_by(|left, right| {
        checkout_score(right, &request)
            .total_cmp(&checkout_score(left, &request))
            .then_with(|| max_confidence(right).total_cmp(&max_confidence(left)))
            .then_with(|| first_anchor(left).cmp(first_anchor(right)))
    });

    let mut total_tokens = 0;
    let mut cells = Vec::new();
    let mut alternatives = Vec::new();
    for cell in candidates {
        let next_total = total_tokens + cell.cost.token_count;
        if next_total <= request.token_budget {
            total_tokens = next_total;
            cells.push(cell);
        } else {
            alternatives.push(CheckoutAlternative {
                cell_id: cell.id,
                reason: CheckoutAlternativeReason::TokenBudgetExceeded,
                citations: citations(&cell),
                max_confidence: Confidence::new(max_confidence(&cell))?,
            });
        }
    }

    let audit_traces = cells
        .iter()
        .map(|cell| {
            let mut trace = audit(cell);
            trace.dependencies = dependency_context_for_dependencies(kernel, &trace.dependencies)?;
            trace.revision_links = revision_links_for_cell(kernel, cell.id)?;
            trace.revision_context =
                revision_context_for_links(kernel, cell.id, &trace.revision_links)?;
            Ok(trace)
        })
        .collect::<Result<Vec<_>, CheckoutError>>()?;
    let uncertainty = cells
        .iter()
        .map(|cell| {
            Ok(UncertaintyEntry {
                cell_id: cell.id,
                max_confidence: Confidence::new(max_confidence(cell))?,
                uncertainty_score: cell.uncertainty.score,
                surprise_bits: cell.uncertainty.surprise_bits,
                rationale: (!cell.uncertainty.rationale.is_empty())
                    .then(|| cell.uncertainty.rationale.clone()),
            })
        })
        .collect::<Result<Vec<_>, CheckoutError>>()?;
    let context_packets = compile_context_packets(&cells, &audit_traces, &request)?;
    let frontier_recommendations = cells
        .iter()
        .filter(|cell| cell.activation == ActivationState::Frontier)
        .map(|cell| FrontierRecommendation {
            cell_id: cell.id,
            citations: citations(cell),
        })
        .collect::<Vec<_>>();
    let summary = checkout_summary(
        total_tokens,
        request.token_budget,
        &audit_traces,
        &uncertainty,
        &context_packets,
        &frontier_recommendations,
        &alternatives,
    );

    Ok(CheckoutSlice {
        cells,
        total_tokens,
        summary,
        audit_traces,
        uncertainty,
        context_packets,
        frontier_recommendations,
        alternatives,
    })
}

/// Converts deterministic checkout constraints into storage lookup constraints.
pub fn cell_lookup_from_checkout_request(request: &CheckoutRequest) -> CellLookup {
    CellLookup {
        semantic_anchor: request
            .semantic_anchor
            .as_ref()
            .map(|anchor| anchor.as_str().to_string()),
        scope: request.scope.clone(),
        valid_at: request.valid_at,
        system_at: request.system_at,
        commit_id: request.commit_id,
        activation: request.activation,
        lifecycle_stage: request.lifecycle_stage,
        retention_policy: request.retention_policy,
        use_policy: request.use_policy,
        promotion_policy: request.promotion_policy,
        projection_kind: request.projection_kind,
        minimum_uncertainty: request.minimum_uncertainty,
        minimum_surprise_bits: request.minimum_surprise_bits,
        minimum_probability_delta: request.minimum_probability_delta,
        minimum_salience: request.minimum_salience,
        minimum_context_affordance: request.minimum_context_affordance,
        minimum_epistemic_pressure: request.minimum_epistemic_pressure,
        context_gap_kind: request.context_gap_kind,
        minimum_context_gap_priority: request.minimum_context_gap_priority,
        invalidation_condition_kind: request.invalidation_condition_kind,
        minimum_invalidation_priority: request.minimum_invalidation_priority,
        epistemic_action: request.epistemic_action,
        epistemic_action_reason: request.epistemic_action_reason,
        selection_reason: request.selection_reason,
        trajectory_memory_strategy: request.trajectory_memory_strategy,
        minimum_trajectory_memory_confidence: request.minimum_trajectory_memory_confidence,
        answerability_question: request.answerability_question.clone(),
        evidence_source: request.evidence_source.clone(),
        dependency_target: request.dependency_target,
        dependency_kind: request.dependency_kind,
        minimum_confidence: Some(request.minimum_confidence),
        ..CellLookup::default()
    }
}

/// Produces a basic audit trace for a StateCell.
pub fn audit(cell: &StateCell) -> AuditTrace {
    AuditTrace {
        cell_id: cell.id,
        commit_id: cell.commit_id,
        citations: citations(cell),
        evidence: cell
            .evidence
            .iter()
            .map(|evidence| AuditEvidence {
                source: evidence.source.as_str().to_string(),
                locator: evidence.citation.locator.clone(),
                confidence: evidence.confidence,
                trust: evidence.trust.clone(),
            })
            .collect(),
        dependencies: cell
            .dependencies
            .iter()
            .map(|dependency| AuditDependency {
                target: dependency.target,
                kind: dependency.kind,
                rationale: dependency.rationale.clone(),
                anchors: Vec::new(),
                citations: Vec::new(),
            })
            .collect(),
        revision_links: Vec::new(),
        revision_context: Vec::new(),
    }
}

fn checkout_summary(
    total_tokens: i64,
    token_budget: i64,
    audit_traces: &[AuditTrace],
    uncertainty: &[UncertaintyEntry],
    context_packets: &[ContextPacket],
    frontier_recommendations: &[FrontierRecommendation],
    alternatives: &[CheckoutAlternative],
) -> CheckoutSummary {
    let mut confidence_values = uncertainty
        .iter()
        .map(|entry| entry.max_confidence)
        .collect::<Vec<_>>();
    confidence_values.sort_by(|left, right| left.value().total_cmp(&right.value()));
    CheckoutSummary {
        selected_cell_count: audit_traces.len(),
        alternative_count: alternatives.len(),
        total_tokens,
        token_budget,
        citation_count: audit_traces.iter().map(|trace| trace.citations.len()).sum(),
        uncertainty_count: uncertainty.len(),
        context_packet_count: context_packets.len(),
        frontier_recommendation_count: frontier_recommendations.len(),
        revision_link_count: audit_traces
            .iter()
            .map(|trace| trace.revision_links.len())
            .sum(),
        revision_context_count: audit_traces
            .iter()
            .map(|trace| trace.revision_context.len())
            .sum(),
        epistemic_action_counts: epistemic_action_counts(context_packets),
        epistemic_action_reason_counts: epistemic_action_reason_counts(context_packets),
        selection_reason_counts: selection_reason_counts(context_packets),
        epistemic_pressure: epistemic_pressure_summary(context_packets),
        maximum_context_affordance_score: maximum_context_affordance_score(context_packets),
        maximum_salience_score: maximum_salience_score(context_packets),
        context_gap_count: context_gap_count(context_packets),
        context_gap_kind_counts: context_gap_kind_counts(context_packets),
        maximum_context_gap_priority: maximum_context_gap_priority(context_packets),
        invalidation_condition_count: invalidation_condition_count(context_packets),
        invalidation_condition_kind_counts: invalidation_condition_kind_counts(context_packets),
        maximum_invalidation_priority: maximum_invalidation_priority(context_packets),
        minimum_selected_confidence: confidence_values.first().copied(),
        maximum_selected_confidence: confidence_values.last().copied(),
        bounded_by_token_budget: total_tokens <= token_budget,
    }
}

fn maximum_context_affordance_score(context_packets: &[ContextPacket]) -> f32 {
    context_packets
        .iter()
        .filter_map(|packet| packet.selection.as_ref())
        .map(|selection| selection.context_affordance_score)
        .fold(0.0, f32::max)
}

fn maximum_salience_score(context_packets: &[ContextPacket]) -> f32 {
    context_packets
        .iter()
        .filter_map(|packet| packet.selection.as_ref())
        .map(|selection| selection.salience_score)
        .fold(0.0, f32::max)
}

fn context_gap_count(context_packets: &[ContextPacket]) -> usize {
    context_packets
        .iter()
        .filter_map(|packet| packet.selection.as_ref())
        .map(|selection| selection.context_gaps.len())
        .sum()
}

fn context_gap_kind_counts(context_packets: &[ContextPacket]) -> Vec<ContextGapKindCount> {
    let mut counts = Vec::new();
    for kind in [
        ContextGapKind::MissingEvidence,
        ContextGapKind::MissingDecision,
        ContextGapKind::MissingConstraint,
        ContextGapKind::MissingDependency,
    ] {
        let count = context_packets
            .iter()
            .filter_map(|packet| packet.selection.as_ref())
            .flat_map(|selection| selection.context_gaps.iter())
            .filter(|gap| gap.kind == kind)
            .count();
        if count > 0 {
            counts.push(ContextGapKindCount { kind, count });
        }
    }
    counts
}

fn maximum_context_gap_priority(context_packets: &[ContextPacket]) -> Option<Confidence> {
    context_packets
        .iter()
        .filter_map(|packet| packet.selection.as_ref())
        .flat_map(|selection| selection.context_gaps.iter())
        .map(|gap| gap.priority)
        .max_by(|left, right| left.value().total_cmp(&right.value()))
}

fn invalidation_condition_count(context_packets: &[ContextPacket]) -> usize {
    context_packets
        .iter()
        .filter_map(|packet| packet.selection.as_ref())
        .map(|selection| selection.invalidation_conditions.len())
        .sum()
}

fn invalidation_condition_kind_counts(
    context_packets: &[ContextPacket],
) -> Vec<InvalidationConditionKindCount> {
    let mut counts = Vec::new();
    for kind in [
        InvalidationConditionKind::ContradictoryEvidence,
        InvalidationConditionKind::BoundaryViolation,
        InvalidationConditionKind::TemporalExpiry,
        InvalidationConditionKind::DependencyInvalidated,
    ] {
        let count = context_packets
            .iter()
            .filter_map(|packet| packet.selection.as_ref())
            .flat_map(|selection| selection.invalidation_conditions.iter())
            .filter(|condition| condition.kind == kind)
            .count();
        if count > 0 {
            counts.push(InvalidationConditionKindCount { kind, count });
        }
    }
    counts
}

fn maximum_invalidation_priority(context_packets: &[ContextPacket]) -> Option<Confidence> {
    context_packets
        .iter()
        .filter_map(|packet| packet.selection.as_ref())
        .flat_map(|selection| selection.invalidation_conditions.iter())
        .map(|condition| condition.priority)
        .max_by(|left, right| left.value().total_cmp(&right.value()))
}

fn epistemic_pressure_summary(context_packets: &[ContextPacket]) -> EpistemicPressureSummary {
    context_packets
        .iter()
        .filter_map(|packet| packet.selection.as_ref())
        .map(|selection| selection.epistemic_pressure)
        .fold(
            EpistemicPressureSummary::default(),
            |mut summary, pressure: EpistemicPressure| {
                summary.maximum_revision_pressure = summary
                    .maximum_revision_pressure
                    .max(pressure.revision_pressure);
                summary.maximum_scavenging_pressure = summary
                    .maximum_scavenging_pressure
                    .max(pressure.scavenging_pressure);
                summary.maximum_checkout_pressure = summary
                    .maximum_checkout_pressure
                    .max(pressure.checkout_pressure);
                summary
            },
        )
}

fn epistemic_action_counts(context_packets: &[ContextPacket]) -> Vec<EpistemicActionCount> {
    let mut counts = Vec::new();
    for action in [
        EpistemicAction::Use,
        EpistemicAction::Hedge,
        EpistemicAction::Verify,
        EpistemicAction::Revise,
        EpistemicAction::Scavenge,
    ] {
        let count = context_packets
            .iter()
            .filter(|packet| {
                packet
                    .selection
                    .as_ref()
                    .map(|selection| selection.epistemic_action == action)
                    .unwrap_or(action == EpistemicAction::Use)
            })
            .count();
        if count > 0 {
            counts.push(EpistemicActionCount { action, count });
        }
    }
    counts
}

fn epistemic_action_reason_counts(
    context_packets: &[ContextPacket],
) -> Vec<EpistemicActionReasonCount> {
    let mut counts = Vec::new();
    for reason in [
        EpistemicActionReason::HighUncertainty,
        EpistemicActionReason::HighSurprise,
        EpistemicActionReason::ModerateUncertainty,
        EpistemicActionReason::LowEvidenceConfidence,
        EpistemicActionReason::MiscalibratedConfidence,
    ] {
        let count = context_packets
            .iter()
            .filter(|packet| {
                packet
                    .selection
                    .as_ref()
                    .map(|selection| selection.epistemic_action_reasons.contains(&reason))
                    .unwrap_or(false)
            })
            .count();
        if count > 0 {
            counts.push(EpistemicActionReasonCount { reason, count });
        }
    }
    counts
}

fn selection_reason_counts(context_packets: &[ContextPacket]) -> Vec<SelectionReasonCount> {
    let mut counts = Vec::new();
    for reason in [
        ContextPacketSelectionReason::EvidenceConfidence,
        ContextPacketSelectionReason::UtilityFeedback,
        ContextPacketSelectionReason::LifecycleStage,
        ContextPacketSelectionReason::ProjectionProfile,
        ContextPacketSelectionReason::NativeUncertainty,
        ContextPacketSelectionReason::EpistemicCalibration,
        ContextPacketSelectionReason::ContextAffordance,
        ContextPacketSelectionReason::ContextGap,
        ContextPacketSelectionReason::InvalidationCondition,
        ContextPacketSelectionReason::TrajectoryMemory,
        ContextPacketSelectionReason::LifecyclePolicy,
        ContextPacketSelectionReason::AttentionSignal,
        ContextPacketSelectionReason::Answerability,
    ] {
        let count = context_packets
            .iter()
            .filter(|packet| {
                packet
                    .selection
                    .as_ref()
                    .map(|selection| selection.reasons.contains(&reason))
                    .unwrap_or(false)
            })
            .count();
        if count > 0 {
            counts.push(SelectionReasonCount { reason, count });
        }
    }
    counts
}

fn revision_links_for_cell<K: StorageKernel>(
    kernel: &K,
    cell_id: StateCellId,
) -> Result<Vec<RevisionLinkRecord>, CheckoutError> {
    let mut links = kernel.list_revision_links(RevisionLinkLookup {
        source: Some(cell_id),
        ..RevisionLinkLookup::default()
    })?;
    for link in kernel.list_revision_links(RevisionLinkLookup {
        target: Some(cell_id),
        ..RevisionLinkLookup::default()
    })? {
        if !links.contains(&link) {
            links.push(link);
        }
    }
    Ok(links)
}

fn cell_matches_revision_constraints<K: StorageKernel>(
    kernel: &K,
    cell_id: StateCellId,
    related_cell: Option<StateCellId>,
    revision_kind: Option<RevisionLinkKind>,
) -> Result<bool, CheckoutError> {
    if related_cell.is_none() && revision_kind.is_none() {
        return Ok(true);
    }
    Ok(revision_links_for_cell(kernel, cell_id)?
        .iter()
        .any(|link| {
            revision_kind.map_or(true, |kind| link.kind == kind)
                && related_cell.map_or(true, |related| {
                    (link.source == cell_id && link.target == related)
                        || (link.target == cell_id && link.source == related)
                })
        }))
}

fn revision_context_for_links<K: StorageKernel>(
    kernel: &K,
    selected_cell_id: StateCellId,
    links: &[RevisionLinkRecord],
) -> Result<Vec<AuditRevisionContext>, CheckoutError> {
    let mut contexts = Vec::new();
    for link in links {
        let (related_cell_id, relation) =
            if link.source == selected_cell_id && link.target == selected_cell_id {
                (selected_cell_id, AuditRevisionRelation::SelfLink)
            } else if link.source == selected_cell_id {
                (link.target, AuditRevisionRelation::SourceToTarget)
            } else {
                (link.source, AuditRevisionRelation::TargetFromSource)
            };
        if contexts
            .iter()
            .any(|context: &AuditRevisionContext| context.related_cell_id == related_cell_id)
        {
            continue;
        }
        if let Some(related_cell) = kernel
            .lookup_cells(CellLookup {
                cell_id: Some(related_cell_id),
                ..CellLookup::default()
            })?
            .into_iter()
            .next()
        {
            contexts.push(AuditRevisionContext {
                selected_cell_id,
                related_cell_id,
                relation,
                kind: link.kind,
                anchors: related_cell.anchors.clone(),
                citations: citations(&related_cell),
                max_confidence: Confidence::new(max_confidence(&related_cell))?,
                activation: related_cell.activation,
            });
        }
    }
    Ok(contexts)
}

fn dependency_context_for_dependencies<K: StorageKernel>(
    kernel: &K,
    dependencies: &[AuditDependency],
) -> Result<Vec<AuditDependency>, CheckoutError> {
    let mut contexts = Vec::new();
    for dependency in dependencies {
        let target_cell = kernel
            .lookup_cells(CellLookup {
                cell_id: Some(dependency.target),
                ..CellLookup::default()
            })?
            .into_iter()
            .next();
        let anchors = target_cell
            .as_ref()
            .map(|target_cell| target_cell.anchors.clone())
            .unwrap_or_default();
        let citations = target_cell.as_ref().map(citations).unwrap_or_default();
        contexts.push(AuditDependency {
            target: dependency.target,
            kind: dependency.kind,
            rationale: dependency.rationale.clone(),
            anchors,
            citations,
        });
    }
    Ok(contexts)
}

fn context_packet_dependency_context(context: &AuditDependency) -> ContextPacketDependencyContext {
    ContextPacketDependencyContext {
        target: context.target,
        kind: context.kind,
        rationale: context.rationale.clone(),
        anchors: context.anchors.clone(),
        citations: context.citations.clone(),
    }
}

fn context_packet_revision_context(context: &AuditRevisionContext) -> ContextPacketRevisionContext {
    ContextPacketRevisionContext {
        related_cell_id: context.related_cell_id,
        relation: match context.relation {
            AuditRevisionRelation::SourceToTarget => ContextPacketRevisionRelation::SourceToTarget,
            AuditRevisionRelation::TargetFromSource => {
                ContextPacketRevisionRelation::TargetFromSource
            }
            AuditRevisionRelation::SelfLink => ContextPacketRevisionRelation::SelfLink,
        },
        kind: context.kind,
        anchors: context.anchors.clone(),
        citations: context.citations.clone(),
        max_confidence: context.max_confidence,
        activation: context.activation,
    }
}

fn append_revision_context_citations(packet: &mut ContextPacket) {
    for context in &packet.revision_context {
        for citation in &context.citations {
            if !packet.citations.contains(citation) {
                packet.citations.push(citation.clone());
            }
        }
    }
}

fn append_dependency_context_citations(packet: &mut ContextPacket) {
    for context in &packet.dependency_context {
        for citation in &context.citations {
            if !packet.citations.contains(citation) {
                packet.citations.push(citation.clone());
            }
        }
    }
}

fn compile_context_packets(
    cells: &[StateCell],
    audit_traces: &[AuditTrace],
    request: &CheckoutRequest,
) -> Result<Vec<ContextPacket>, CheckoutError> {
    cells
        .iter()
        .zip(audit_traces.iter())
        .map(|(cell, trace)| compile_context_packet(cell, trace, request))
        .collect()
}

fn compile_context_packet(
    cell: &StateCell,
    trace: &AuditTrace,
    request: &CheckoutRequest,
) -> Result<ContextPacket, CheckoutError> {
    let mut packet = cell.context_packet(request.context_profile, cell.cost.token_count);
    packet.dependency_context = trace
        .dependencies
        .iter()
        .map(context_packet_dependency_context)
        .collect();
    append_dependency_context_citations(&mut packet);
    packet.revision_context = trace
        .revision_context
        .iter()
        .map(context_packet_revision_context)
        .collect();
    append_revision_context_citations(&mut packet);
    suppress_invalidated_trajectory_memory(&mut packet, cell, request);
    let decision = context_compiler_decision(cell, trace, request)?;
    let plan = CheckoutPacketPlan::from_decision(decision);
    packet.compiler_policy = plan.decision.policy;
    packet.strategy = plan.decision.strategy;
    packet.abstraction_level = plan.decision.abstraction_level;
    packet.compiler_reason_tags = plan.reason_tags();
    packet.compiler_evidence_locators =
        supported_compiler_evidence_locators(&packet, &plan.decision);
    let plan_evidence_locators = if packet.compiler_evidence_locators.is_empty() {
        packet.citations.clone()
    } else {
        packet.compiler_evidence_locators.clone()
    };
    if let Ok(public_plan) = plan.to_context_packet_plan(&plan_evidence_locators) {
        packet.plan = Some(public_plan);
    }
    prepend_compiler_lines(&mut packet, cell, &plan.decision, request.token_budget);
    Ok(packet)
}

struct CheckoutPacketPlan {
    decision: ContextCompilerDecision,
    purpose: ContextPacketPurpose,
    required_contracts: Vec<ContextPacketRequirement>,
}

impl CheckoutPacketPlan {
    fn from_decision(decision: ContextCompilerDecision) -> Self {
        let purpose = packet_plan_purpose(&decision);
        let required_contracts = packet_plan_required_contracts(&decision);
        Self {
            decision,
            purpose,
            required_contracts,
        }
    }

    fn reason_tags(&self) -> Vec<String> {
        let mut tags = self.decision.reason_tags.clone();
        push_unique_reason_tag(
            &mut tags,
            match self.decision.policy {
                ContextCompilerPolicy::RawBaseline => "packet-plan-stage:raw-baseline",
                ContextCompilerPolicy::Automatic => "packet-plan-stage:automatic",
                ContextCompilerPolicy::ModelAssisted => "packet-plan-stage:model-assisted",
            },
        );
        push_unique_reason_tag(&mut tags, packet_plan_purpose_tag(self.purpose));
        for contract in &self.required_contracts {
            push_unique_reason_tag(&mut tags, packet_plan_contract_tag(*contract));
        }
        tags
    }

    fn to_context_packet_plan(
        &self,
        evidence_locators: &[String],
    ) -> Result<ContextPacketPlan, CoreError> {
        ContextPacketPlan::new(
            self.purpose,
            self.required_contracts.clone(),
            Vec::new(),
            self.decision.abstraction_level,
            self.decision.strategy,
            self.reason_tags(),
            evidence_locators.to_vec(),
        )
    }
}

fn packet_plan_purpose(decision: &ContextCompilerDecision) -> ContextPacketPurpose {
    if decision
        .reason_tags
        .iter()
        .any(|tag| tag == "lifecycle-safe-use-policy" || tag == "task-intent-safety")
    {
        ContextPacketPurpose::SafetyGuidance
    } else if decision
        .reason_tags
        .iter()
        .any(|tag| tag == "trajectory-applicability-match")
    {
        ContextPacketPurpose::TrajectoryReuse
    } else if decision.strategy == ContextPacketStrategy::RevisionCapsule {
        ContextPacketPurpose::RevisionGuidance
    } else if decision.strategy == ContextPacketStrategy::UncertaintyBrief {
        ContextPacketPurpose::UncertaintyGuidance
    } else if decision.strategy == ContextPacketStrategy::FalsificationBrief {
        ContextPacketPurpose::Falsification
    } else if decision.abstraction_level == ContextAbstractionLevel::EvidenceDense {
        ContextPacketPurpose::Audit
    } else {
        ContextPacketPurpose::AnswerSupport
    }
}

fn packet_plan_required_contracts(
    decision: &ContextCompilerDecision,
) -> Vec<ContextPacketRequirement> {
    let mut contracts = Vec::new();
    if decision.include_revision_line {
        contracts.push(ContextPacketRequirement::RevisionContext);
    }
    if decision.include_uncertainty_line {
        contracts.push(ContextPacketRequirement::Uncertainty);
    }
    if decision.include_invalidation_lines {
        contracts.push(ContextPacketRequirement::Invalidation);
    }
    if decision.include_context_affordance_line {
        contracts.push(ContextPacketRequirement::ContextAffordance);
    }
    if decision
        .reason_tags
        .iter()
        .any(|tag| tag == "lifecycle-safe-use-policy")
    {
        contracts.push(ContextPacketRequirement::LifecyclePolicy);
    }
    if decision
        .reason_tags
        .iter()
        .any(|tag| tag == "trajectory-applicability-match")
    {
        contracts.push(ContextPacketRequirement::TrajectoryApplicability);
    }
    if !decision.evidence_locators.is_empty() {
        contracts.push(ContextPacketRequirement::EvidenceCitations);
    }
    contracts
}

fn packet_plan_purpose_tag(purpose: ContextPacketPurpose) -> &'static str {
    match purpose {
        ContextPacketPurpose::AnswerSupport => "packet-plan-purpose:answer-support",
        ContextPacketPurpose::RevisionGuidance => "packet-plan-purpose:revision-guidance",
        ContextPacketPurpose::SafetyGuidance => "packet-plan-purpose:safety-guidance",
        ContextPacketPurpose::TrajectoryReuse => "packet-plan-purpose:trajectory-reuse",
        ContextPacketPurpose::UncertaintyGuidance => "packet-plan-purpose:uncertainty-guidance",
        ContextPacketPurpose::Falsification => "packet-plan-purpose:falsification",
        ContextPacketPurpose::Audit => "packet-plan-purpose:audit",
    }
}

fn packet_plan_contract_tag(contract: ContextPacketRequirement) -> &'static str {
    match contract {
        ContextPacketRequirement::RevisionContext => "packet-plan-contract:revision-context",
        ContextPacketRequirement::LifecyclePolicy => "packet-plan-hard-gate:lifecycle-policy",
        ContextPacketRequirement::Uncertainty => "packet-plan-contract:uncertainty",
        ContextPacketRequirement::Invalidation => "packet-plan-hard-gate:invalidation",
        ContextPacketRequirement::TrajectoryApplicability => {
            "packet-plan-hard-gate:trajectory-applicability"
        }
        ContextPacketRequirement::ContextAffordance => "packet-plan-hard-gate:context-affordance",
        ContextPacketRequirement::EvidenceCitations => "packet-plan-contract:evidence-citations",
    }
}

fn push_unique_reason_tag(tags: &mut Vec<String>, tag: &'static str) {
    if !tags.iter().any(|existing| existing == tag) {
        tags.push(tag.to_string());
    }
}

struct ContextCompilerDecision {
    policy: ContextCompilerPolicy,
    strategy: ContextPacketStrategy,
    abstraction_level: ContextAbstractionLevel,
    include_revision_line: bool,
    include_uncertainty_line: bool,
    include_calibration_line: bool,
    include_context_affordance_line: bool,
    include_dependency_lines: bool,
    include_gap_lines: bool,
    include_invalidation_lines: bool,
    safety_guidance_first: bool,
    reason_tags: Vec<String>,
    evidence_locators: Vec<String>,
    proposed_lines: Vec<ContextCompilerProposalLine>,
}

#[derive(Clone, Debug, PartialEq)]
struct CompilerPacketAddition {
    text: String,
    source: ContextPacketEntrySource,
    confidence: Confidence,
    token_count: i64,
}

impl CompilerPacketAddition {
    fn new(
        text: impl Into<String>,
        source: ContextPacketEntrySource,
        confidence: Confidence,
        token_count: i64,
    ) -> Self {
        Self {
            text: text.into(),
            source,
            confidence,
            token_count,
        }
    }

    fn into_entry(self) -> ContextPacketEntry {
        ContextPacketEntry {
            source: self.source,
            text: self.text,
            confidence: self.confidence,
            token_count: self.token_count,
        }
    }
}

fn context_compiler_decision(
    cell: &StateCell,
    trace: &AuditTrace,
    request: &CheckoutRequest,
) -> Result<ContextCompilerDecision, CheckoutError> {
    let baseline_strategy = baseline_context_packet_strategy(cell, trace);
    if request.compiler_policy == ContextCompilerPolicy::Automatic {
        return Ok(automatic_context_compiler_decision(
            cell,
            trace,
            request,
            baseline_strategy,
        ));
    }

    if request.compiler_policy == ContextCompilerPolicy::ModelAssisted {
        return model_assisted_context_compiler_decision(request, cell);
    }

    Ok(baseline_context_compiler_decision(
        request.compiler_policy,
        cell,
        baseline_strategy,
    ))
}

fn baseline_context_compiler_decision(
    policy: ContextCompilerPolicy,
    cell: &StateCell,
    strategy: ContextPacketStrategy,
) -> ContextCompilerDecision {
    ContextCompilerDecision {
        policy,
        strategy,
        abstraction_level: abstraction_level_for_strategy(strategy),
        include_revision_line: strategy == ContextPacketStrategy::RevisionCapsule,
        include_uncertainty_line: strategy == ContextPacketStrategy::RevisionCapsule
            && cell.uncertainty.is_recorded(),
        include_calibration_line: cell.calibration.is_recorded(),
        include_context_affordance_line: false,
        include_dependency_lines: false,
        include_gap_lines: !cell.context_gaps.is_empty(),
        include_invalidation_lines: !cell.invalidation_conditions.is_empty(),
        safety_guidance_first: false,
        reason_tags: vec!["raw-baseline".to_string()],
        evidence_locators: Vec::new(),
        proposed_lines: Vec::new(),
    }
}

fn model_assisted_context_compiler_decision(
    request: &CheckoutRequest,
    cell: &StateCell,
) -> Result<ContextCompilerDecision, CheckoutError> {
    let mut matching_proposals = request
        .compiler_proposals
        .iter()
        .filter(|proposal| proposal.target_cell_id == cell.id);
    let proposal = matching_proposals
        .next()
        .ok_or(CheckoutError::MissingContextCompilerProposal)?;
    if matching_proposals.next().is_some() {
        return Err(CheckoutError::AmbiguousContextCompilerProposal);
    }

    Ok(ContextCompilerDecision {
        policy: ContextCompilerPolicy::ModelAssisted,
        strategy: proposal.strategy,
        abstraction_level: proposal.abstraction_level,
        include_revision_line: proposal.strategy == ContextPacketStrategy::RevisionCapsule,
        include_uncertainty_line: matches!(
            proposal.strategy,
            ContextPacketStrategy::RevisionCapsule | ContextPacketStrategy::UncertaintyBrief
        ) && cell.uncertainty.is_recorded(),
        include_calibration_line: cell.calibration.is_recorded(),
        include_context_affordance_line: false,
        include_dependency_lines: false,
        include_gap_lines: !cell.context_gaps.is_empty(),
        include_invalidation_lines: !cell.invalidation_conditions.is_empty(),
        safety_guidance_first: matches!(
            proposal.strategy,
            ContextPacketStrategy::ScavengingBrief | ContextPacketStrategy::FalsificationBrief
        ),
        reason_tags: proposal.reason_tags.clone(),
        evidence_locators: proposal.evidence_locators.clone(),
        proposed_lines: proposal.proposed_lines.clone(),
    })
}

fn automatic_context_compiler_decision(
    cell: &StateCell,
    trace: &AuditTrace,
    request: &CheckoutRequest,
    baseline_strategy: ContextPacketStrategy,
) -> ContextCompilerDecision {
    let task_signal = CompilerTaskSignal::from_request(request);

    if lifecycle_policy_requires_safe_guidance(cell) && task_signal.asks_for_action {
        return ContextCompilerDecision {
            policy: ContextCompilerPolicy::Automatic,
            strategy: ContextPacketStrategy::FalsificationBrief,
            abstraction_level: ContextAbstractionLevel::Falsification,
            include_revision_line: false,
            include_uncertainty_line: cell.uncertainty.is_recorded(),
            include_calibration_line: cell.calibration.is_recorded(),
            include_context_affordance_line: false,
            include_dependency_lines: false,
            include_gap_lines: !cell.context_gaps.is_empty(),
            include_invalidation_lines: !cell.invalidation_conditions.is_empty(),
            safety_guidance_first: true,
            reason_tags: vec![
                "task-intent-action".to_string(),
                "lifecycle-safe-use-policy".to_string(),
            ],
            evidence_locators: native_compiler_evidence_locators(cell),
            proposed_lines: Vec::new(),
        };
    }

    if !cell.invalidation_conditions.is_empty() && task_signal.asks_for_action {
        return ContextCompilerDecision {
            policy: ContextCompilerPolicy::Automatic,
            strategy: ContextPacketStrategy::FalsificationBrief,
            abstraction_level: ContextAbstractionLevel::Falsification,
            include_revision_line: true,
            include_uncertainty_line: false,
            include_calibration_line: cell.calibration.is_recorded(),
            include_context_affordance_line: false,
            include_dependency_lines: false,
            include_gap_lines: !cell.context_gaps.is_empty(),
            include_invalidation_lines: true,
            safety_guidance_first: true,
            reason_tags: vec![
                "task-intent-action".to_string(),
                "invalidation-condition-present".to_string(),
            ],
            evidence_locators: native_compiler_evidence_locators(cell),
            proposed_lines: Vec::new(),
        };
    }

    if !cell.context_gaps.is_empty() && task_signal.asks_for_safety {
        return ContextCompilerDecision {
            policy: ContextCompilerPolicy::Automatic,
            strategy: ContextPacketStrategy::ScavengingBrief,
            abstraction_level: ContextAbstractionLevel::Scavenging,
            include_revision_line: false,
            include_uncertainty_line: false,
            include_calibration_line: cell.calibration.is_recorded(),
            include_context_affordance_line: false,
            include_dependency_lines: false,
            include_gap_lines: true,
            include_invalidation_lines: !cell.invalidation_conditions.is_empty(),
            safety_guidance_first: true,
            reason_tags: vec![
                "task-intent-safety".to_string(),
                "context-gap-present".to_string(),
            ],
            evidence_locators: native_compiler_evidence_locators(cell),
            proposed_lines: Vec::new(),
        };
    }

    if !trace.revision_context.is_empty() && task_signal.asks_for_change {
        return ContextCompilerDecision {
            policy: ContextCompilerPolicy::Automatic,
            strategy: ContextPacketStrategy::RevisionCapsule,
            abstraction_level: ContextAbstractionLevel::Capsule,
            include_revision_line: true,
            include_uncertainty_line: cell.uncertainty.is_recorded(),
            include_calibration_line: cell.calibration.is_recorded(),
            include_context_affordance_line: false,
            include_dependency_lines: false,
            include_gap_lines: !cell.context_gaps.is_empty(),
            include_invalidation_lines: !cell.invalidation_conditions.is_empty(),
            safety_guidance_first: false,
            reason_tags: vec![
                "task-intent-change".to_string(),
                "revision-context-present".to_string(),
            ],
            evidence_locators: revision_context_compiler_evidence_locators(cell, trace),
            proposed_lines: Vec::new(),
        };
    }

    if !trace.dependencies.is_empty() && task_signal.asks_for_safety {
        return ContextCompilerDecision {
            policy: ContextCompilerPolicy::Automatic,
            strategy: if baseline_strategy == ContextPacketStrategy::RawProjection {
                ContextPacketStrategy::OperationalBrief
            } else {
                baseline_strategy
            },
            abstraction_level: ContextAbstractionLevel::EvidenceDense,
            include_revision_line: baseline_strategy == ContextPacketStrategy::RevisionCapsule,
            include_uncertainty_line: cell.uncertainty.is_recorded(),
            include_calibration_line: cell.calibration.is_recorded(),
            include_context_affordance_line: false,
            include_dependency_lines: true,
            include_gap_lines: !cell.context_gaps.is_empty(),
            include_invalidation_lines: !cell.invalidation_conditions.is_empty(),
            safety_guidance_first: false,
            reason_tags: vec![
                "task-intent-safety".to_string(),
                "dependency-context-present".to_string(),
            ],
            evidence_locators: dependency_context_compiler_evidence_locators(cell, trace),
            proposed_lines: Vec::new(),
        };
    }

    if context_affordance_requires_evidence_dense_guidance(cell) {
        return ContextCompilerDecision {
            policy: ContextCompilerPolicy::Automatic,
            strategy: if baseline_strategy == ContextPacketStrategy::RawProjection {
                ContextPacketStrategy::OperationalBrief
            } else {
                baseline_strategy
            },
            abstraction_level: ContextAbstractionLevel::EvidenceDense,
            include_revision_line: baseline_strategy == ContextPacketStrategy::RevisionCapsule,
            include_uncertainty_line: cell.uncertainty.is_recorded(),
            include_calibration_line: cell.calibration.is_recorded(),
            include_context_affordance_line: true,
            include_dependency_lines: false,
            include_gap_lines: !cell.context_gaps.is_empty(),
            include_invalidation_lines: !cell.invalidation_conditions.is_empty(),
            safety_guidance_first: false,
            reason_tags: vec![
                "context-affordance-risk".to_string(),
                "evidence-dense-affordance-guidance".to_string(),
            ],
            evidence_locators: native_compiler_evidence_locators(cell),
            proposed_lines: Vec::new(),
        };
    }

    if epistemic_pressure_requires_revision_guidance(cell) {
        return ContextCompilerDecision {
            policy: ContextCompilerPolicy::Automatic,
            strategy: ContextPacketStrategy::RevisionCapsule,
            abstraction_level: ContextAbstractionLevel::Capsule,
            include_revision_line: !trace.revision_context.is_empty(),
            include_uncertainty_line: cell.uncertainty.is_recorded(),
            include_calibration_line: cell.calibration.is_recorded(),
            include_context_affordance_line: false,
            include_dependency_lines: false,
            include_gap_lines: !cell.context_gaps.is_empty(),
            include_invalidation_lines: !cell.invalidation_conditions.is_empty(),
            safety_guidance_first: true,
            reason_tags: vec![
                "epistemic-revision-pressure".to_string(),
                "native-pressure-signal".to_string(),
            ],
            evidence_locators: native_compiler_evidence_locators(cell),
            proposed_lines: Vec::new(),
        };
    }

    if epistemic_pressure_requires_scavenging_guidance(cell) {
        return ContextCompilerDecision {
            policy: ContextCompilerPolicy::Automatic,
            strategy: ContextPacketStrategy::ScavengingBrief,
            abstraction_level: ContextAbstractionLevel::Scavenging,
            include_revision_line: false,
            include_uncertainty_line: cell.uncertainty.is_recorded(),
            include_calibration_line: cell.calibration.is_recorded(),
            include_context_affordance_line: false,
            include_dependency_lines: false,
            include_gap_lines: !cell.context_gaps.is_empty(),
            include_invalidation_lines: !cell.invalidation_conditions.is_empty(),
            safety_guidance_first: true,
            reason_tags: vec![
                "epistemic-scavenging-pressure".to_string(),
                "native-pressure-signal".to_string(),
            ],
            evidence_locators: native_compiler_evidence_locators(cell),
            proposed_lines: Vec::new(),
        };
    }

    if let Some(trajectory_memory) = &cell.trajectory_memory {
        if trajectory_applicability_score_for_signal(trajectory_memory, &task_signal) > 0.0 {
            return ContextCompilerDecision {
                policy: ContextCompilerPolicy::Automatic,
                strategy: trajectory_memory.checkout_strategy,
                abstraction_level: trajectory_memory
                    .checkout_strategy
                    .default_abstraction_level(),
                include_revision_line: false,
                include_uncertainty_line: cell.uncertainty.is_recorded(),
                include_calibration_line: cell.calibration.is_recorded(),
                include_context_affordance_line: false,
                include_dependency_lines: false,
                include_gap_lines: !cell.context_gaps.is_empty(),
                include_invalidation_lines: !cell.invalidation_conditions.is_empty(),
                safety_guidance_first: matches!(
                    trajectory_memory.checkout_strategy,
                    ContextPacketStrategy::ScavengingBrief
                        | ContextPacketStrategy::FalsificationBrief
                ),
                reason_tags: vec![
                    "task-intent-trajectory-reuse".to_string(),
                    "trajectory-applicability-match".to_string(),
                ],
                evidence_locators: trajectory_compiler_evidence_locators(cell, trajectory_memory),
                proposed_lines: Vec::new(),
            };
        }
    }

    if matches!(request.context_profile, ContextProfile::Audit) {
        return ContextCompilerDecision {
            policy: ContextCompilerPolicy::Automatic,
            strategy: baseline_strategy,
            abstraction_level: ContextAbstractionLevel::EvidenceDense,
            include_revision_line: baseline_strategy == ContextPacketStrategy::RevisionCapsule,
            include_uncertainty_line: cell.uncertainty.is_recorded(),
            include_calibration_line: cell.calibration.is_recorded(),
            include_context_affordance_line: false,
            include_dependency_lines: false,
            include_gap_lines: !cell.context_gaps.is_empty(),
            include_invalidation_lines: !cell.invalidation_conditions.is_empty(),
            safety_guidance_first: false,
            reason_tags: vec!["audit-profile".to_string()],
            evidence_locators: native_compiler_evidence_locators(cell),
            proposed_lines: Vec::new(),
        };
    }

    baseline_context_compiler_decision(ContextCompilerPolicy::Automatic, cell, baseline_strategy)
}

fn baseline_context_packet_strategy(cell: &StateCell, trace: &AuditTrace) -> ContextPacketStrategy {
    if !trace.revision_context.is_empty()
        || matches!(
            cell.lifecycle_stage,
            LifecycleStage::Superseded | LifecycleStage::Contradicted
        )
    {
        ContextPacketStrategy::RevisionCapsule
    } else if cell.uncertainty.is_recorded()
        || cell.calibration.is_recorded()
        || matches!(
            cell.epistemic_action(),
            EpistemicAction::Verify | EpistemicAction::Revise | EpistemicAction::Scavenge
        )
    {
        ContextPacketStrategy::UncertaintyBrief
    } else if !cell.context_gaps.is_empty() {
        ContextPacketStrategy::ScavengingBrief
    } else if !cell.invalidation_conditions.is_empty() {
        ContextPacketStrategy::FalsificationBrief
    } else if let Some(trajectory_memory) = &cell.trajectory_memory {
        trajectory_memory.checkout_strategy
    } else if !cell.projections.is_empty() {
        ContextPacketStrategy::OperationalBrief
    } else {
        ContextPacketStrategy::RawProjection
    }
}

fn abstraction_level_for_strategy(strategy: ContextPacketStrategy) -> ContextAbstractionLevel {
    strategy.default_abstraction_level()
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

#[derive(Clone, Debug, Default)]
struct CompilerTaskSignal {
    normalized_intent: String,
    intent_tokens: HashSet<String>,
    asks_for_action: bool,
    asks_for_safety: bool,
    asks_for_change: bool,
    asks_for_readiness: bool,
    asks_for_debugging: bool,
    asks_for_reflection: bool,
    token_pressure: bool,
}

impl CompilerTaskSignal {
    fn from_request(request: &CheckoutRequest) -> Self {
        let intent = request
            .compiler_intent
            .as_deref()
            .or(request.answerability_question.as_deref())
            .unwrap_or("");
        Self::from_text(intent, request.context_profile, request.token_budget)
    }

    fn from_text(intent: &str, context_profile: ContextProfile, token_budget: i64) -> Self {
        let normalized_intent = intent.to_ascii_lowercase();
        let intent_tokens = normalized_keyword_set(&normalized_intent);
        let asks_for_action =
            contains_any(
                &normalized_intent,
                &[
                    "what should i do",
                    "action",
                    "next",
                    "retry",
                    "deploy",
                    "release",
                    "execute",
                ],
            ) || contains_token(&intent_tokens, &["action", "next", "retry", "deploy"]);
        let asks_for_safety = contains_any(
            &normalized_intent,
            &[
                "safe",
                "known",
                "ready",
                "verified",
                "verify",
                "should i trust",
            ],
        ) || contains_token(
            &intent_tokens,
            &["safe", "known", "ready", "verified", "verify"],
        );
        let asks_for_change = contains_any(
            &normalized_intent,
            &["changed", "what changed", "why", "superseded", "regressed"],
        ) || contains_token(
            &intent_tokens,
            &["changed", "why", "superseded", "regressed"],
        );
        let asks_for_readiness = contains_token(
            &intent_tokens,
            &["ready", "readiness", "blocked", "unblocked"],
        );
        let asks_for_debugging = context_profile == ContextProfile::Debugging
            || contains_token(&intent_tokens, &["debug", "debugging", "failure", "failed"]);
        let asks_for_reflection = context_profile == ContextProfile::Reflection
            || contains_token(
                &intent_tokens,
                &["lesson", "reuse", "repeat", "retrospective"],
            );
        let token_pressure = token_budget <= 32;

        Self {
            normalized_intent,
            intent_tokens,
            asks_for_action,
            asks_for_safety,
            asks_for_change,
            asks_for_readiness,
            asks_for_debugging,
            asks_for_reflection,
            token_pressure,
        }
    }

    fn has_intent(&self) -> bool {
        !self.normalized_intent.trim().is_empty() && !self.intent_tokens.is_empty()
    }
}

fn contains_token(tokens: &HashSet<String>, needles: &[&str]) -> bool {
    needles.iter().any(|needle| tokens.contains(*needle))
}

fn prepend_compiler_lines(
    packet: &mut ContextPacket,
    cell: &StateCell,
    decision: &ContextCompilerDecision,
    token_budget: i64,
) {
    if matches!(decision.strategy, ContextPacketStrategy::RawProjection) {
        return;
    }
    if matches!(decision.strategy, ContextPacketStrategy::OperationalBrief)
        && decision.proposed_lines.is_empty()
        && !decision.include_dependency_lines
        && !decision.include_context_affordance_line
    {
        return;
    }

    let mut additions = Vec::new();
    if decision.safety_guidance_first {
        append_lifecycle_policy_lines(&mut additions, packet, cell);
        append_invalidation_lines(&mut additions, cell);
        append_gap_lines(&mut additions, cell);
    }
    let uncertainty_guidance_first =
        decision.safety_guidance_first && decision.include_uncertainty_line;
    if uncertainty_guidance_first {
        append_uncertainty_line(&mut additions, cell);
    }
    if decision.include_context_affordance_line {
        append_context_affordance_lines(&mut additions, packet, cell);
    }
    if decision.include_dependency_lines {
        append_dependency_lines(&mut additions, packet);
    }
    append_supported_proposed_lines(&mut additions, packet, decision);
    if let Some(projection) = cell
        .projections_by_kind(MemoryProjectionKind::Semantic)
        .first()
    {
        additions.push(CompilerPacketAddition::new(
            projection.text.clone(),
            ContextPacketEntrySource::Projection(projection.kind),
            projection.confidence,
            projection.cost.token_count,
        ));
    }
    if decision.include_revision_line {
        append_revision_lines(&mut additions, packet);
    }
    if decision.include_calibration_line {
        additions.push(CompilerPacketAddition::new(
            format!(
                "Calibration warning: expected confidence {:.3}, observed frequency {:.3}, error {:.3}; {}",
                cell.calibration.expected_confidence.value(),
                cell.calibration.observed_frequency.value(),
                cell.calibration.calibration_error,
                cell.calibration.rationale
            ),
            ContextPacketEntrySource::NativeCalibration,
            cell.calibration.observed_frequency,
            7,
        ));
    }
    if decision.include_uncertainty_line && !uncertainty_guidance_first {
        append_uncertainty_line(&mut additions, cell);
    }
    if !decision.safety_guidance_first {
        if decision.include_gap_lines {
            append_gap_lines(&mut additions, cell);
        }
        if decision.include_invalidation_lines {
            append_invalidation_lines(&mut additions, cell);
        }
    }

    materialize_packet_lines(packet, additions, token_budget);
}

fn append_supported_proposed_lines(
    additions: &mut Vec<CompilerPacketAddition>,
    packet: &ContextPacket,
    decision: &ContextCompilerDecision,
) {
    if decision.proposed_lines.is_empty() {
        return;
    }

    let packet_citations = packet.citations.iter().collect::<HashSet<_>>();
    for proposed_line in &decision.proposed_lines {
        if proposed_line
            .citations
            .iter()
            .all(|citation| packet_citations.contains(citation))
        {
            additions.push(CompilerPacketAddition::new(
                proposed_line.text.clone(),
                ContextPacketEntrySource::ModelAssistedCompilerLine,
                Confidence::default(),
                proposed_line.token_count,
            ));
        }
    }
}

fn supported_compiler_evidence_locators(
    packet: &ContextPacket,
    decision: &ContextCompilerDecision,
) -> Vec<String> {
    let packet_citations = packet.citations.iter().collect::<HashSet<_>>();
    decision
        .evidence_locators
        .iter()
        .filter(|locator| packet_citations.contains(locator))
        .cloned()
        .collect()
}

fn append_revision_lines(additions: &mut Vec<CompilerPacketAddition>, packet: &ContextPacket) {
    for context in &packet.revision_context {
        let relation = match context.relation {
            ContextPacketRevisionRelation::SourceToTarget => "Supersedes prior belief",
            ContextPacketRevisionRelation::TargetFromSource => "Revises from related belief",
            ContextPacketRevisionRelation::SelfLink => "Self-revision belief link",
        };
        let related = context
            .anchors
            .first()
            .map(|anchor| anchor.as_str().to_string())
            .unwrap_or_else(|| context.related_cell_id.to_string());
        additions.push(CompilerPacketAddition::new(
            format!(
                "{} {} via {:?}; prior confidence {:.3}",
                relation,
                related,
                context.kind,
                context.max_confidence.value()
            ),
            ContextPacketEntrySource::NativeRevisionContext,
            context.max_confidence,
            5,
        ));
    }
}

fn append_dependency_lines(additions: &mut Vec<CompilerPacketAddition>, packet: &ContextPacket) {
    for context in &packet.dependency_context {
        let target = context
            .anchors
            .first()
            .map(|anchor| anchor.as_str().to_string())
            .unwrap_or_else(|| context.target.to_string());
        additions.push(CompilerPacketAddition::new(
            format!(
                "Dependency context: {:?} {}; target {}",
                context.kind, context.rationale, target
            ),
            ContextPacketEntrySource::NativeDependencyContext,
            Confidence::default(),
            5,
        ));
    }
}

fn append_gap_lines(additions: &mut Vec<CompilerPacketAddition>, cell: &StateCell) {
    let mut gaps = cell.context_gaps.iter().collect::<Vec<_>>();
    gaps.sort_by(|left, right| right.priority.value().total_cmp(&left.priority.value()));
    for gap in gaps {
        additions.push(CompilerPacketAddition::new(
            format!("Missing context: {}", gap.question),
            ContextPacketEntrySource::NativeContextGap,
            gap.priority,
            4,
        ));
    }
}

fn append_invalidation_lines(additions: &mut Vec<CompilerPacketAddition>, cell: &StateCell) {
    let mut conditions = cell.invalidation_conditions.iter().collect::<Vec<_>>();
    conditions.sort_by(|left, right| right.priority.value().total_cmp(&left.priority.value()));
    for condition in conditions {
        additions.push(CompilerPacketAddition::new(
            format!("Invalidation condition: {}", condition.condition),
            ContextPacketEntrySource::NativeInvalidationCondition,
            condition.priority,
            4,
        ));
    }
}

fn append_uncertainty_line(additions: &mut Vec<CompilerPacketAddition>, cell: &StateCell) {
    additions.push(CompilerPacketAddition::new(
        format!(
            "Uncertainty warning: score {:.3}, surprise {:.3} bits; {}",
            cell.uncertainty.score.value(),
            cell.uncertainty.surprise_bits,
            cell.uncertainty.rationale
        ),
        ContextPacketEntrySource::NativeUncertainty,
        cell.uncertainty.score,
        5,
    ));
}

fn suppress_invalidated_trajectory_memory(
    packet: &mut ContextPacket,
    cell: &StateCell,
    request: &CheckoutRequest,
) {
    let Some(memory) = &cell.trajectory_memory else {
        return;
    };
    let task_signal = CompilerTaskSignal::from_request(request);
    if !trajectory_invalidated_by_task_signal(memory, &task_signal) {
        return;
    }

    let removed_lines = packet
        .entries
        .iter()
        .filter(|entry| entry.source == ContextPacketEntrySource::NativeTrajectoryMemory)
        .map(|entry| entry.text.clone())
        .collect::<Vec<_>>();
    if removed_lines.is_empty() {
        return;
    }
    packet
        .entries
        .retain(|entry| entry.source != ContextPacketEntrySource::NativeTrajectoryMemory);
    packet.lines.retain(|line| !removed_lines.contains(line));
    if let Some(selection) = packet.selection.as_mut() {
        selection.trajectory_memory = None;
        selection
            .reasons
            .retain(|reason| *reason != ContextPacketSelectionReason::TrajectoryMemory);
    }
    packet.token_count = packet.entries.iter().map(|entry| entry.token_count).sum();
}

fn append_lifecycle_policy_lines(
    additions: &mut Vec<CompilerPacketAddition>,
    packet: &ContextPacket,
    cell: &StateCell,
) {
    for entry in &packet.entries {
        if entry.source == ContextPacketEntrySource::NativeLifecyclePolicy {
            additions.push(CompilerPacketAddition::new(
                entry.text.clone(),
                entry.source,
                entry.confidence,
                entry.token_count,
            ));
        }
    }
    if additions
        .iter()
        .any(|addition| addition.source == ContextPacketEntrySource::NativeLifecyclePolicy)
    {
        return;
    }
    let evaluation = cell.context_lifecycle_evaluation();
    if evaluation == Default::default() {
        return;
    }
    additions.push(CompilerPacketAddition::new(
        checkout_lifecycle_policy_line(&evaluation),
        ContextPacketEntrySource::NativeLifecyclePolicy,
        Confidence::new(max_confidence(cell)).unwrap_or_default(),
        10,
    ));
}

fn append_context_affordance_lines(
    additions: &mut Vec<CompilerPacketAddition>,
    packet: &ContextPacket,
    cell: &StateCell,
) {
    for entry in &packet.entries {
        if entry.source == ContextPacketEntrySource::NativeContextAffordance {
            additions.push(CompilerPacketAddition::new(
                entry.text.clone(),
                entry.source,
                entry.confidence,
                entry.token_count,
            ));
        }
    }
    if additions
        .iter()
        .any(|addition| addition.source == ContextPacketEntrySource::NativeContextAffordance)
    {
        return;
    }
    if !cell.context_affordance.is_recorded() {
        return;
    }
    additions.push(CompilerPacketAddition::new(
        checkout_context_affordance_line(cell),
        ContextPacketEntrySource::NativeContextAffordance,
        Confidence::new(
            cell.context_affordance
                .context_affordance_score()
                .clamp(0.0, 1.0),
        )
        .unwrap_or_default(),
        12,
    ));
}

fn checkout_context_affordance_line(cell: &StateCell) -> String {
    let affordance = &cell.context_affordance;
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

fn checkout_lifecycle_policy_line(evaluation: &ContextLifecycleEvaluation) -> String {
    format!(
        "Lifecycle policy: {}; {}; {}",
        checkout_retention_policy_text(evaluation.retention),
        checkout_use_policy_text(evaluation.use_policy),
        checkout_promotion_policy_text(evaluation.promotion)
    )
}

fn checkout_retention_policy_text(policy: RetentionPolicy) -> &'static str {
    match policy {
        RetentionPolicy::Persistent => "persistent retention",
        RetentionPolicy::DecayUnlessReinforced => "decay unless reinforced",
        RetentionPolicy::Ephemeral => "ephemeral retention",
    }
}

fn checkout_use_policy_text(policy: UsePolicy) -> &'static str {
    match policy {
        UsePolicy::UseDirectly => "use directly",
        UsePolicy::HedgeBeforeUse => "hedge before use",
        UsePolicy::VerifyBeforeUse => "verify before use",
        UsePolicy::DoNotUseForAnswer => "do not use for answer",
    }
}

fn checkout_promotion_policy_text(policy: PromotionPolicy) -> String {
    match policy {
        PromotionPolicy::Manual => "manual promotion".to_string(),
        PromotionPolicy::EvidenceCount(count) => {
            format!("promote after {count} supporting evidence records")
        }
        PromotionPolicy::ConfidenceThreshold(confidence) => {
            format!("promote above confidence {:.3}", confidence.value())
        }
        PromotionPolicy::RepeatedObservation(count) => {
            format!("promote after {count} repeated observations")
        }
    }
}

fn materialize_packet_lines(
    packet: &mut ContextPacket,
    additions: Vec<CompilerPacketAddition>,
    token_budget: i64,
) {
    let existing_lines = std::mem::take(&mut packet.lines);
    let existing_entries = packet.entries.clone();
    let mut lines = Vec::new();
    let mut entries = Vec::new();
    let mut token_count = 0;
    for addition in additions {
        if token_count + addition.token_count <= token_budget && !lines.contains(&addition.text) {
            token_count += addition.token_count;
            lines.push(addition.text.clone());
            entries.push(addition.into_entry());
        }
    }
    for line in existing_lines {
        let entry = existing_entries
            .iter()
            .find(|entry| entry.text == line)
            .cloned();
        let cost = entry.as_ref().map(|entry| entry.token_count).unwrap_or(1);
        if token_count + cost <= token_budget && !lines.contains(&line) {
            token_count += cost;
            lines.push(line);
            if let Some(entry) = entry {
                entries.push(entry);
            }
        }
    }
    packet.lines = lines;
    packet.entries = entries;
    packet.token_count = token_count;
}

fn citations(cell: &StateCell) -> Vec<String> {
    cell.evidence
        .iter()
        .map(|evidence| evidence.citation.locator.clone())
        .collect()
}

fn native_compiler_evidence_locators(cell: &StateCell) -> Vec<String> {
    citations(cell)
}

fn revision_context_compiler_evidence_locators(
    cell: &StateCell,
    trace: &AuditTrace,
) -> Vec<String> {
    let mut locators = native_compiler_evidence_locators(cell);
    for context in &trace.revision_context {
        for citation in &context.citations {
            if !locators.contains(citation) {
                locators.push(citation.clone());
            }
        }
    }
    locators
}

fn dependency_context_compiler_evidence_locators(
    cell: &StateCell,
    trace: &AuditTrace,
) -> Vec<String> {
    let mut locators = native_compiler_evidence_locators(cell);
    for dependency in &trace.dependencies {
        for citation in &dependency.citations {
            if !locators.contains(citation) {
                locators.push(citation.clone());
            }
        }
    }
    locators
}

fn trajectory_compiler_evidence_locators(
    cell: &StateCell,
    trajectory_memory: &TrajectoryMemory,
) -> Vec<String> {
    let mut locators = native_compiler_evidence_locators(cell);
    if !trajectory_memory.trace_locator.trim().is_empty()
        && !locators.contains(&trajectory_memory.trace_locator)
    {
        locators.push(trajectory_memory.trace_locator.clone());
    }
    locators
}

fn max_confidence(cell: &StateCell) -> f32 {
    cell.evidence
        .iter()
        .map(|evidence| evidence.confidence.value())
        .fold(0.0, f32::max)
}

fn checkout_score(cell: &StateCell, request: &CheckoutRequest) -> f32 {
    (max_confidence(cell)
        + cell.utility_feedback.utility_score()
        + cell.attention.salience_score()
        + cell.context_affordance.context_affordance_score())
        / 4.0
        + trajectory_selection_score(cell, request)
}

fn trajectory_selection_score(cell: &StateCell, request: &CheckoutRequest) -> f32 {
    let Some(memory) = &cell.trajectory_memory else {
        return 0.0;
    };
    let Some(answerability_question) = request.answerability_question.as_deref() else {
        return 0.0;
    };
    let task_signal = CompilerTaskSignal::from_text(
        answerability_question,
        request.context_profile,
        request.token_budget,
    );
    trajectory_applicability_score_for_signal(memory, &task_signal)
}

fn trajectory_applicability_score_for_signal(
    memory: &TrajectoryMemory,
    task_signal: &CompilerTaskSignal,
) -> f32 {
    if !task_signal.has_intent() || trajectory_invalidated_by_task_signal(memory, task_signal) {
        return 0.0;
    }

    let applicability_overlap = memory
        .applicability_conditions
        .iter()
        .map(|condition| overlap_with_intent(condition, &task_signal.intent_tokens))
        .fold(0.0, f32::max);
    let lesson_overlap =
        overlap_with_intent(&memory.reusable_lesson, &task_signal.intent_tokens) * 0.75;
    let failure_overlap = overlap_with_intent(&memory.failure_mode, &task_signal.intent_tokens)
        * if task_signal.asks_for_debugging {
            0.75
        } else {
            0.5
        };
    let reflection_bonus = if task_signal.asks_for_reflection {
        0.15
    } else {
        0.0
    };
    let readiness_bonus = if task_signal.asks_for_readiness {
        0.05
    } else {
        0.0
    };
    let pressure_penalty = if task_signal.token_pressure
        && memory.checkout_strategy == ContextPacketStrategy::RawProjection
    {
        0.1
    } else {
        0.0
    };
    let best_overlap = (applicability_overlap
        .max(lesson_overlap)
        .max(failure_overlap)
        + reflection_bonus
        + readiness_bonus
        - pressure_penalty)
        .clamp(0.0, 1.0);
    if best_overlap == 0.0 {
        0.0
    } else {
        memory.confidence.value() * best_overlap
    }
}

fn trajectory_invalidated_by_task_signal(
    memory: &TrajectoryMemory,
    task_signal: &CompilerTaskSignal,
) -> bool {
    memory
        .invalidation_conditions
        .iter()
        .any(|condition| overlap_with_intent(condition, &task_signal.intent_tokens) >= 0.6)
}

fn overlap_with_intent(text: &str, intent_tokens: &HashSet<String>) -> f32 {
    let tokens = normalized_keyword_set(text);
    if tokens.is_empty() || intent_tokens.is_empty() {
        return 0.0;
    }
    let overlap = tokens
        .iter()
        .filter(|token| intent_tokens.contains(*token))
        .count();
    overlap as f32 / tokens.len() as f32
}

fn lifecycle_policy_requires_safe_guidance(cell: &StateCell) -> bool {
    matches!(
        cell.lifecycle_policy.use_policy,
        UsePolicy::HedgeBeforeUse | UsePolicy::VerifyBeforeUse | UsePolicy::DoNotUseForAnswer
    )
}

fn context_affordance_requires_evidence_dense_guidance(cell: &StateCell) -> bool {
    cell.context_affordance.is_recorded()
        && (cell.context_affordance.risk_of_misuse >= 0.8
            || cell.context_affordance.ambiguity >= 0.8)
}

fn epistemic_pressure_requires_scavenging_guidance(cell: &StateCell) -> bool {
    let pressure = cell.epistemic_pressure();
    pressure.scavenging_pressure >= 0.65
        && pressure.scavenging_pressure >= pressure.revision_pressure
}

fn epistemic_pressure_requires_revision_guidance(cell: &StateCell) -> bool {
    let pressure = cell.epistemic_pressure();
    pressure.revision_pressure >= 0.65 && pressure.revision_pressure > pressure.scavenging_pressure
}

fn normalized_keyword_set(text: &str) -> HashSet<String> {
    text.split(|character: char| !character.is_ascii_alphanumeric())
        .filter_map(|token| {
            let token = token.trim().to_ascii_lowercase();
            (token.len() > 2).then_some(token)
        })
        .collect()
}

fn first_anchor(cell: &StateCell) -> &str {
    cell.anchors
        .first()
        .map(SemanticAnchor::as_str)
        .unwrap_or("")
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use chrono::{DateTime, TimeZone, Utc};
    use continuitydb_core::{
        ActivationState, Answerability, AttentionSignal, CellCost, CellDependency,
        CellDependencyKind, CellPayload, Citation, CommitId, Confidence, ContextAbstractionLevel,
        ContextAffordance, ContextCompilerPolicy, ContextCompilerProposal,
        ContextCompilerProposalLine, ContextGap, ContextGapKind, ContextLifecyclePolicy,
        ContextPacketEntrySource, ContextPacketPurpose, ContextPacketRequirement,
        ContextPacketSelectionReason, ContextPacketStrategy, ContextProfile, EpistemicAction,
        EpistemicActionReason, EpistemicCalibration, EpistemicExpectation, EpistemicUncertainty,
        Evidence, InvalidationCondition, InvalidationConditionKind, LifecycleStage,
        MemoryProjection, MemoryProjectionKind, PromotionPolicy, RetentionPolicy, RevisionLinkKind,
        RevisionLinkRecord, Scope, SemanticAnchor, SourceId, StateCell, StateCellId,
        SystemTimeRange, TrajectoryMemory, TrustSignal, UsePolicy, UtilityFeedback, ValidTimeRange,
    };
    use continuitydb_kernel::{CellLookup, KernelError, StorageKernel};
    use continuitydb_memory::MemoryKernel;

    use super::{
        audit, cell_lookup_from_checkout_request, checkout, CheckoutError, CheckoutRequest,
    };

    fn sample_cell(
        anchor: &str,
        confidence: f32,
        tokens: i64,
    ) -> Result<StateCell, Box<dyn std::error::Error>> {
        sample_cell_with_question_and_source(
            anchor,
            "what should the agent know?",
            "test",
            confidence,
            tokens,
        )
    }

    fn sample_cell_with_question_and_source(
        anchor: &str,
        question: &str,
        source: &str,
        confidence: f32,
        tokens: i64,
    ) -> Result<StateCell, Box<dyn std::error::Error>> {
        let valid_from = Utc
            .with_ymd_and_hms(2026, 5, 20, 0, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        StateCell::new(
            StateCellId::new(),
            vec![SemanticAnchor::new(anchor)],
            ValidTimeRange::new(valid_from, None)?,
            Scope::Project("continuitydb".to_string()),
            Answerability::new(vec![question.to_string()])?,
            vec![Evidence {
                source: SourceId::new(source),
                citation: Citation {
                    locator: format!("test://{anchor}"),
                },
                confidence: Confidence::new(confidence)?,
                trust: vec![TrustSignal::DirectObservation],
            }],
            CellPayload::Text(anchor.to_string()),
            CellCost::new(tokens, 0)?,
        )
        .map_err(Into::into)
    }

    #[derive(Default)]
    struct RecordingKernel {
        lookup: RefCell<Option<CellLookup>>,
    }

    impl StorageKernel for RecordingKernel {
        fn append_cells_at_with_commit_id<I>(
            &mut self,
            _cells: I,
            _committed_at: DateTime<Utc>,
            _commit_id: CommitId,
        ) -> Result<(), KernelError>
        where
            I: IntoIterator<Item = StateCell>,
        {
            Ok(())
        }

        fn lookup_cells(&self, lookup: CellLookup) -> Result<Vec<StateCell>, KernelError> {
            *self.lookup.borrow_mut() = Some(lookup);
            Ok(Vec::new())
        }

        fn lookup_commit_manifest(
            &self,
            _commit_id: CommitId,
        ) -> Result<Option<continuitydb_core::CommitManifest>, KernelError> {
            Ok(None)
        }

        fn list_commit_manifests(
            &self,
        ) -> Result<Vec<continuitydb_core::CommitManifest>, KernelError> {
            Ok(Vec::new())
        }

        fn list_commit_manifests_matching(
            &self,
            _lookup: continuitydb_kernel::CommitManifestLookup,
        ) -> Result<Vec<continuitydb_core::CommitManifest>, KernelError> {
            Ok(Vec::new())
        }

        fn append_revision_link(
            &mut self,
            _revision_link: continuitydb_core::RevisionLinkRecord,
        ) -> Result<(), KernelError> {
            Ok(())
        }

        fn list_revision_links(
            &self,
            _lookup: continuitydb_kernel::RevisionLinkLookup,
        ) -> Result<Vec<continuitydb_core::RevisionLinkRecord>, KernelError> {
            Ok(Vec::new())
        }
    }

    fn test_commit_time() -> Result<DateTime<Utc>, Box<dyn std::error::Error>> {
        Utc.with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp").into())
    }

    fn append_committed(
        kernel: &mut MemoryKernel,
        mut cell: StateCell,
    ) -> Result<StateCell, Box<dyn std::error::Error>> {
        let committed_at = test_commit_time()?;
        let commit_id = CommitId::new();
        kernel.append_cell_at_with_commit_id(cell.clone(), committed_at, commit_id)?;
        cell.system_time = SystemTimeRange::open_from(committed_at);
        cell.commit_id = commit_id;
        Ok(cell)
    }

    #[test]
    fn checkout_pushes_semantic_constraints_to_kernel() -> Result<(), Box<dyn std::error::Error>> {
        let kernel = RecordingKernel::default();
        checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: Some(Scope::Project("continuitydb".to_string())),
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
                minimum_salience: Some(0.7),
                minimum_context_affordance: None,
                minimum_epistemic_pressure: None,
                context_gap_kind: Some(ContextGapKind::MissingEvidence),
                minimum_context_gap_priority: Some(Confidence::new(0.7)?),
                invalidation_condition_kind: None,
                minimum_invalidation_priority: None,
                epistemic_action: None,
                epistemic_action_reason: None,
                selection_reason: Some(ContextPacketSelectionReason::AttentionSignal),
                trajectory_memory_strategy: Some(ContextPacketStrategy::FalsificationBrief),
                minimum_trajectory_memory_confidence: Some(Confidence::new(0.8)?),
                answerability_question: Some("what is frontier?".to_string()),
                compiler_intent: None,
                compiler_proposals: Vec::new(),
                evidence_source: Some("human".to_string()),
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Debugging,
                compiler_policy: ContextCompilerPolicy::RawBaseline,
                minimum_confidence: Confidence::new(0.8)?,
                token_budget: 10,
            },
        )?;

        let lookup = kernel
            .lookup
            .borrow()
            .clone()
            .ok_or_else(|| std::io::Error::other("lookup was not captured"))?;
        assert_eq!(
            lookup.scope,
            Some(Scope::Project("continuitydb".to_string()))
        );
        assert_eq!(
            lookup.answerability_question,
            Some("what is frontier?".to_string())
        );
        assert_eq!(lookup.evidence_source, Some("human".to_string()));
        assert_eq!(lookup.minimum_confidence, Some(Confidence::new(0.8)?));
        assert_eq!(lookup.minimum_salience, Some(0.7));
        assert_eq!(
            lookup.context_gap_kind,
            Some(ContextGapKind::MissingEvidence)
        );
        assert_eq!(
            lookup.minimum_context_gap_priority,
            Some(Confidence::new(0.7)?)
        );
        assert_eq!(
            lookup.selection_reason,
            Some(ContextPacketSelectionReason::AttentionSignal)
        );
        assert_eq!(
            lookup.trajectory_memory_strategy,
            Some(ContextPacketStrategy::FalsificationBrief)
        );
        assert_eq!(
            lookup.minimum_trajectory_memory_confidence,
            Some(Confidence::new(0.8)?)
        );
        Ok(())
    }

    #[test]
    fn checkout_request_builds_storage_lookup_without_token_budget(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let semantic_anchor = SemanticAnchor::new("project:continuitydb:lookup");
        let request = CheckoutRequest {
            semantic_anchor: Some(semantic_anchor.clone()),
            scope: Some(Scope::Project("continuitydb".to_string())),
            valid_at: Some(test_commit_time()?),
            system_at: Some(test_commit_time()?),
            commit_id: Some(CommitId::new()),
            activation: Some(ActivationState::Frontier),
            lifecycle_stage: Some(LifecycleStage::Operationalized),
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
            answerability_question: Some("what changed?".to_string()),
            compiler_intent: None,
            compiler_proposals: Vec::new(),
            evidence_source: Some("source:ops".to_string()),
            dependency_target: Some(StateCellId::from_u128(7)),
            dependency_kind: Some(CellDependencyKind::DerivedFrom),
            revision_related_cell: Some(StateCellId::from_u128(8)),
            revision_link_kind: Some(RevisionLinkKind::DerivesFrom),
            context_profile: ContextProfile::Execution,
            compiler_policy: ContextCompilerPolicy::RawBaseline,
            minimum_confidence: Confidence::new(0.7)?,
            token_budget: 123,
        };

        let lookup = cell_lookup_from_checkout_request(&request);

        assert_eq!(
            lookup.semantic_anchor.as_deref(),
            Some(semantic_anchor.as_str())
        );
        assert_eq!(lookup.scope, request.scope);
        assert_eq!(lookup.valid_at, request.valid_at);
        assert_eq!(lookup.system_at, request.system_at);
        assert_eq!(lookup.commit_id, request.commit_id);
        assert_eq!(lookup.activation, request.activation);
        assert_eq!(lookup.lifecycle_stage, request.lifecycle_stage);
        assert_eq!(
            lookup.answerability_question,
            request.answerability_question
        );
        assert_eq!(lookup.evidence_source, request.evidence_source);
        assert_eq!(lookup.dependency_target, request.dependency_target);
        assert_eq!(lookup.dependency_kind, request.dependency_kind);
        assert_eq!(lookup.minimum_confidence, Some(request.minimum_confidence));
        assert_eq!(lookup.cell_id, None);
        Ok(())
    }

    #[test]
    fn checkout_pushes_semantic_anchor_to_kernel() -> Result<(), Box<dyn std::error::Error>> {
        let kernel = RecordingKernel::default();
        checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: Some(SemanticAnchor::new("project:continuitydb:release-status")),
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
                answerability_question: None,
                compiler_intent: None,
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Debugging,
                compiler_policy: ContextCompilerPolicy::RawBaseline,
                minimum_confidence: Confidence::new(0.8)?,
                token_budget: 10,
            },
        )?;

        let lookup = kernel
            .lookup
            .borrow()
            .clone()
            .ok_or_else(|| std::io::Error::other("lookup was not captured"))?;
        assert_eq!(
            lookup.semantic_anchor.as_deref(),
            Some("project:continuitydb:release-status")
        );
        Ok(())
    }

    #[test]
    fn checkout_pushes_dependency_constraints_to_kernel() -> Result<(), Box<dyn std::error::Error>>
    {
        let kernel = RecordingKernel::default();
        let target = StateCellId::new();
        checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: Some(Scope::Project("continuitydb".to_string())),
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
                answerability_question: None,
                compiler_intent: None,
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: Some(target),
                dependency_kind: Some(CellDependencyKind::DependsOn),
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Execution,
                compiler_policy: ContextCompilerPolicy::RawBaseline,
                minimum_confidence: Confidence::new(0.8)?,
                token_budget: 10,
            },
        )?;

        let lookup = kernel
            .lookup
            .borrow()
            .clone()
            .ok_or_else(|| std::io::Error::other("lookup was not captured"))?;
        assert_eq!(lookup.dependency_target, Some(target));
        assert_eq!(lookup.dependency_kind, Some(CellDependencyKind::DependsOn));
        Ok(())
    }

    #[test]
    fn checkout_pushes_activation_to_kernel() -> Result<(), Box<dyn std::error::Error>> {
        let kernel = RecordingKernel::default();
        checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: None,
                valid_at: None,
                system_at: None,
                commit_id: None,
                activation: Some(ActivationState::Frontier),
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
                answerability_question: None,
                compiler_intent: None,
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Execution,
                compiler_policy: ContextCompilerPolicy::RawBaseline,
                minimum_confidence: Confidence::new(0.8)?,
                token_budget: 10,
            },
        )?;

        let lookup = kernel
            .lookup
            .borrow()
            .clone()
            .ok_or_else(|| std::io::Error::other("lookup was not captured"))?;
        assert_eq!(lookup.activation, Some(ActivationState::Frontier));
        Ok(())
    }

    #[test]
    fn checkout_pushes_system_time_to_kernel() -> Result<(), Box<dyn std::error::Error>> {
        let kernel = RecordingKernel::default();
        let system_at = test_commit_time()?;
        checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: None,
                valid_at: None,
                system_at: Some(system_at),
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
                answerability_question: None,
                compiler_intent: None,
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Execution,
                compiler_policy: ContextCompilerPolicy::RawBaseline,
                minimum_confidence: Confidence::new(0.8)?,
                token_budget: 10,
            },
        )?;

        let lookup = kernel
            .lookup
            .borrow()
            .clone()
            .ok_or_else(|| std::io::Error::other("lookup was not captured"))?;
        assert_eq!(lookup.system_at, Some(system_at));
        Ok(())
    }

    #[test]
    fn checkout_pushes_commit_id_to_kernel() -> Result<(), Box<dyn std::error::Error>> {
        let kernel = RecordingKernel::default();
        let commit_id = CommitId::new();
        checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: None,
                valid_at: None,
                system_at: None,
                commit_id: Some(commit_id),
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
                answerability_question: None,
                compiler_intent: None,
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Execution,
                compiler_policy: ContextCompilerPolicy::RawBaseline,
                minimum_confidence: Confidence::new(0.8)?,
                token_budget: 10,
            },
        )?;

        let lookup = kernel
            .lookup
            .borrow()
            .clone()
            .ok_or_else(|| std::io::Error::other("lookup was not captured"))?;
        assert_eq!(lookup.commit_id, Some(commit_id));
        Ok(())
    }

    #[test]
    fn checkout_respects_token_budget_and_confidence() -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let high = sample_cell("project:continuitydb:high", 0.95, 10)?;
        let low = sample_cell("project:continuitydb:low", 0.40, 10)?;
        let high = append_committed(&mut kernel, high)?;
        append_committed(&mut kernel, low)?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: Some(Scope::Project("continuitydb".to_string())),
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
                answerability_question: None,
                compiler_intent: None,
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Execution,
                compiler_policy: ContextCompilerPolicy::RawBaseline,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 10,
            },
        )?;

        assert_eq!(slice.cells, vec![high]);
        assert_eq!(slice.total_tokens, 10);
        Ok(())
    }

    #[test]
    fn checkout_filters_by_system_time() -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let committed_at = test_commit_time()?;
        let before_commit = Utc
            .with_ymd_and_hms(2026, 5, 20, 11, 59, 59)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let cell = append_committed(
            &mut kernel,
            sample_cell("project:continuitydb:system-checkout", 0.95, 10)?,
        )?;

        let historical = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: Some(Scope::Project("continuitydb".to_string())),
                valid_at: None,
                system_at: Some(before_commit),
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
                answerability_question: None,
                compiler_intent: None,
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Execution,
                compiler_policy: ContextCompilerPolicy::RawBaseline,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 10,
            },
        )?;
        let current = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: Some(Scope::Project("continuitydb".to_string())),
                valid_at: None,
                system_at: Some(committed_at),
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
                answerability_question: None,
                compiler_intent: None,
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Execution,
                compiler_policy: ContextCompilerPolicy::RawBaseline,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 10,
            },
        )?;

        assert!(historical.cells.is_empty());
        assert_eq!(current.cells, vec![cell]);
        Ok(())
    }

    #[test]
    fn checkout_filters_by_commit_id() -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let committed_at = test_commit_time()?;
        let selected_commit_id = CommitId::new();
        let omitted_commit_id = CommitId::new();
        let mut selected = sample_cell("project:continuitydb:selected-commit", 0.95, 10)?;
        let omitted = sample_cell("project:continuitydb:omitted-commit", 0.95, 10)?;
        kernel.append_cell_at_with_commit_id(selected.clone(), committed_at, selected_commit_id)?;
        kernel.append_cell_at_with_commit_id(omitted, committed_at, omitted_commit_id)?;
        selected.system_time = SystemTimeRange::open_from(committed_at);
        selected.commit_id = selected_commit_id;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: Some(Scope::Project("continuitydb".to_string())),
                valid_at: None,
                system_at: None,
                commit_id: Some(selected_commit_id),
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
                answerability_question: None,
                compiler_intent: None,
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Execution,
                compiler_policy: ContextCompilerPolicy::RawBaseline,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 10,
            },
        )?;

        assert_eq!(slice.cells, vec![selected]);
        assert_eq!(slice.total_tokens, 10);
        Ok(())
    }

    #[test]
    fn checkout_prefers_higher_utility_candidate_under_token_budget(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let mut high_confidence_low_utility =
            sample_cell("project:continuitydb:confidence-only", 0.95, 10)?;
        high_confidence_low_utility.utility_feedback = UtilityFeedback::new(
            Confidence::new(0.1)?,
            Confidence::new(0.1)?,
            Confidence::new(0.1)?,
        );
        let mut lower_confidence_high_utility =
            sample_cell("project:continuitydb:useful", 0.80, 10)?;
        lower_confidence_high_utility.utility_feedback = UtilityFeedback::new(
            Confidence::new(1.0)?,
            Confidence::new(1.0)?,
            Confidence::new(1.0)?,
        );
        let high_confidence_low_utility =
            append_committed(&mut kernel, high_confidence_low_utility)?;
        let lower_confidence_high_utility =
            append_committed(&mut kernel, lower_confidence_high_utility)?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: Some(Scope::Project("continuitydb".to_string())),
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
                answerability_question: None,
                compiler_intent: None,
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Execution,
                compiler_policy: ContextCompilerPolicy::RawBaseline,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 10,
            },
        )?;

        assert_eq!(slice.cells, vec![lower_confidence_high_utility]);
        assert_eq!(slice.alternatives.len(), 1);
        assert_eq!(
            slice.alternatives[0].cell_id,
            high_confidence_low_utility.id
        );
        Ok(())
    }

    #[test]
    fn checkout_prefers_higher_attention_salience_under_token_budget(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let routine = sample_cell("project:continuitydb:routine-context", 0.85, 10)?;
        let mut salient = sample_cell("project:continuitydb:salient-context", 0.85, 10)?;
        salient.set_attention(AttentionSignal::new(0.9, 0.8, 0.9, 0.8)?);
        append_committed(&mut kernel, routine)?;
        let salient = append_committed(&mut kernel, salient)?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: Some(Scope::Project("continuitydb".to_string())),
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
                answerability_question: None,
                compiler_intent: None,
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Execution,
                compiler_policy: ContextCompilerPolicy::RawBaseline,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 10,
            },
        )?;

        assert_eq!(slice.cells, vec![salient.clone()]);
        assert_eq!(
            slice.context_packets[0]
                .selection
                .as_ref()
                .map(|selection| selection.salience_score),
            Some(salient.attention.salience_score())
        );
        assert_eq!(
            slice.summary.maximum_salience_score,
            salient.attention.salience_score()
        );
        Ok(())
    }

    #[test]
    fn checkout_prefers_higher_context_affordance_under_token_budget(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let routine = sample_cell("project:continuitydb:routine-affordance", 0.88, 10)?;
        let mut high_affordance =
            sample_cell("project:continuitydb:high-context-affordance", 0.80, 10)?;
        high_affordance
            .set_context_affordance(ContextAffordance::new(0.95, 0.9, 0.8, 0.4, 0.95, 0.1)?);
        append_committed(&mut kernel, routine)?;
        let high_affordance = append_committed(&mut kernel, high_affordance)?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: Some(Scope::Project("continuitydb".to_string())),
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
                answerability_question: None,
                compiler_intent: None,
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Planning,
                compiler_policy: ContextCompilerPolicy::RawBaseline,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 10,
            },
        )?;

        assert_eq!(slice.cells, vec![high_affordance.clone()]);
        assert_eq!(
            slice.context_packets[0]
                .selection
                .as_ref()
                .map(|selection| selection.context_affordance_score),
            Some(
                high_affordance
                    .context_affordance
                    .context_affordance_score()
            )
        );
        assert_eq!(
            slice.summary.maximum_context_affordance_score,
            high_affordance
                .context_affordance
                .context_affordance_score()
        );
        Ok(())
    }

    #[test]
    fn checkout_filters_by_minimum_salience() -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let mut salient = sample_cell("project:continuitydb:salient-checkout", 0.85, 10)?;
        salient.set_attention(AttentionSignal::new(0.9, 0.8, 0.9, 0.8)?);
        let salient = append_committed(&mut kernel, salient)?;
        let routine = sample_cell("project:continuitydb:routine-checkout", 0.95, 10)?;
        append_committed(&mut kernel, routine)?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: Some(Scope::Project("continuitydb".to_string())),
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
                minimum_salience: Some(0.7),
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
                answerability_question: None,
                compiler_intent: None,
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Execution,
                compiler_policy: ContextCompilerPolicy::RawBaseline,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 20,
            },
        )?;

        assert_eq!(slice.cells, vec![salient]);
        Ok(())
    }

    #[test]
    fn checkout_filters_by_minimum_context_affordance() -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let mut high_value = sample_cell("project:continuitydb:affordance-checkout", 0.85, 10)?;
        high_value.set_context_affordance(ContextAffordance::new(0.95, 0.9, 0.8, 0.4, 0.95, 0.1)?);
        let high_value = append_committed(&mut kernel, high_value)?;
        let routine = sample_cell("project:continuitydb:routine-affordance-checkout", 0.95, 10)?;
        append_committed(&mut kernel, routine)?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: Some(Scope::Project("continuitydb".to_string())),
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
                minimum_context_affordance: Some(0.7),
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
                answerability_question: None,
                compiler_intent: None,
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Planning,
                compiler_policy: ContextCompilerPolicy::RawBaseline,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 20,
            },
        )?;

        assert_eq!(slice.cells, vec![high_value]);
        Ok(())
    }

    #[test]
    fn checkout_filters_by_minimum_epistemic_pressure() -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let mut pressured = sample_cell("project:continuitydb:pressured-checkout", 0.8, 10)?;
        pressured.set_uncertainty(EpistemicUncertainty::new(
            Confidence::new(0.7)?,
            4.0,
            "high uncertainty with violated baseline",
        )?);
        pressured.set_attention(AttentionSignal::new(0.8, 0.6, 0.75, 0.4)?);
        let pressured = append_committed(&mut kernel, pressured)?;
        append_committed(
            &mut kernel,
            sample_cell("project:continuitydb:routine-pressure-checkout", 0.95, 10)?,
        )?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: Some(Scope::Project("continuitydb".to_string())),
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
                minimum_epistemic_pressure: Some(0.55),
                context_gap_kind: None,
                minimum_context_gap_priority: None,
                invalidation_condition_kind: None,
                minimum_invalidation_priority: None,
                epistemic_action: None,
                epistemic_action_reason: None,
                selection_reason: None,
                trajectory_memory_strategy: None,
                minimum_trajectory_memory_confidence: None,
                answerability_question: None,
                compiler_intent: None,
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Execution,
                compiler_policy: ContextCompilerPolicy::RawBaseline,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 20,
            },
        )?;

        assert_eq!(slice.cells, vec![pressured]);
        Ok(())
    }

    #[test]
    fn checkout_filters_by_dependency_target_and_kind() -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let target = StateCellId::new();
        let other_target = StateCellId::new();
        let mut dependent = sample_cell("project:continuitydb:dependent", 0.95, 10)?;
        dependent.dependencies.push(CellDependency::new(
            target,
            CellDependencyKind::DependsOn,
            "depends on target",
        ));
        let mut unrelated = sample_cell("project:continuitydb:unrelated", 0.90, 10)?;
        unrelated.dependencies.push(CellDependency::new(
            other_target,
            CellDependencyKind::DependsOn,
            "depends on another target",
        ));
        let mut support = sample_cell("project:continuitydb:support", 0.85, 10)?;
        support.dependencies.push(CellDependency::new(
            target,
            CellDependencyKind::Supports,
            "supports target",
        ));
        let dependent = append_committed(&mut kernel, dependent)?;
        append_committed(&mut kernel, unrelated)?;
        append_committed(&mut kernel, support)?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: Some(Scope::Project("continuitydb".to_string())),
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
                answerability_question: None,
                compiler_intent: None,
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: Some(target),
                dependency_kind: Some(CellDependencyKind::DependsOn),
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Execution,
                compiler_policy: ContextCompilerPolicy::RawBaseline,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 20,
            },
        )?;

        assert_eq!(slice.cells, vec![dependent]);
        assert_eq!(slice.total_tokens, 10);
        Ok(())
    }

    #[test]
    fn checkout_filters_by_semantic_anchor() -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let selected = sample_cell("project:continuitydb:anchor-selected", 0.91, 10)?;
        let unrelated = sample_cell("project:continuitydb:anchor-unrelated", 0.9, 10)?;
        let selected = append_committed(&mut kernel, selected)?;
        append_committed(&mut kernel, unrelated)?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: Some(SemanticAnchor::new("project:continuitydb:anchor-selected")),
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
                answerability_question: None,
                compiler_intent: None,
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Execution,
                compiler_policy: ContextCompilerPolicy::RawBaseline,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 20,
            },
        )?;

        assert_eq!(slice.cells, vec![selected]);
        Ok(())
    }

    #[test]
    fn checkout_filters_by_answerability_question() -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let status = sample_cell_with_question_and_source(
            "project:continuitydb:status",
            "what is status?",
            "test",
            0.95,
            10,
        )?;
        let frontier = sample_cell_with_question_and_source(
            "project:continuitydb:frontier",
            "what is frontier?",
            "test",
            0.90,
            10,
        )?;
        append_committed(&mut kernel, status)?;
        let frontier = append_committed(&mut kernel, frontier)?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: Some(Scope::Project("continuitydb".to_string())),
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
                answerability_question: Some("what is frontier?".to_string()),
                compiler_intent: None,
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Execution,
                compiler_policy: ContextCompilerPolicy::RawBaseline,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 20,
            },
        )?;

        assert_eq!(slice.cells, vec![frontier]);
        assert_eq!(slice.total_tokens, 10);
        Ok(())
    }

    #[test]
    fn checkout_filters_by_evidence_source() -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let observed = sample_cell_with_question_and_source(
            "project:continuitydb:observed",
            "what is status?",
            "sensor",
            0.95,
            10,
        )?;
        let reviewed = sample_cell_with_question_and_source(
            "project:continuitydb:reviewed",
            "what is status?",
            "human",
            0.90,
            10,
        )?;
        append_committed(&mut kernel, observed)?;
        let reviewed = append_committed(&mut kernel, reviewed)?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: Some(Scope::Project("continuitydb".to_string())),
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
                answerability_question: None,
                compiler_intent: None,
                compiler_proposals: Vec::new(),
                evidence_source: Some("human".to_string()),
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Execution,
                compiler_policy: ContextCompilerPolicy::RawBaseline,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 20,
            },
        )?;

        assert_eq!(slice.cells, vec![reviewed]);
        assert_eq!(slice.total_tokens, 10);
        Ok(())
    }

    #[test]
    fn checkout_filters_by_activation_state() -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let active = sample_cell("project:continuitydb:activation-active", 0.91, 10)?;
        let mut frontier = sample_cell("project:continuitydb:activation-frontier", 0.9, 10)?;
        frontier.activation = ActivationState::Frontier;
        append_committed(&mut kernel, active)?;
        let frontier = append_committed(&mut kernel, frontier)?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: Some(Scope::Project("continuitydb".to_string())),
                valid_at: None,
                system_at: None,
                commit_id: None,
                activation: Some(ActivationState::Frontier),
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
                answerability_question: None,
                compiler_intent: None,
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Execution,
                compiler_policy: ContextCompilerPolicy::RawBaseline,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 20,
            },
        )?;

        assert_eq!(slice.cells, vec![frontier]);
        Ok(())
    }

    #[test]
    fn checkout_filters_by_lifecycle_stage() -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let operationalized = append_committed(
            &mut kernel,
            sample_cell("project:continuitydb:lifecycle-operationalized", 0.95, 10)?
                .with_lifecycle_stage(LifecycleStage::Operationalized),
        )?;
        let _observed = append_committed(
            &mut kernel,
            sample_cell("project:continuitydb:lifecycle-observed", 0.95, 10)?,
        )?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: Some(Scope::Project("continuitydb".to_string())),
                valid_at: None,
                system_at: None,
                commit_id: None,
                activation: None,
                lifecycle_stage: Some(LifecycleStage::Operationalized),
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
                answerability_question: None,
                compiler_intent: None,
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Execution,
                compiler_policy: ContextCompilerPolicy::RawBaseline,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 20,
            },
        )?;

        assert_eq!(slice.cells, vec![operationalized]);
        Ok(())
    }

    #[test]
    fn checkout_filters_by_lifecycle_use_policy() -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let mut verify = sample_cell("project:continuitydb:lifecycle-use-verify", 0.95, 10)?;
        verify.set_lifecycle_policy(ContextLifecyclePolicy {
            retention: RetentionPolicy::DecayUnlessReinforced,
            use_policy: UsePolicy::VerifyBeforeUse,
            promotion: PromotionPolicy::Manual,
        });
        let verify = append_committed(&mut kernel, verify)?;
        append_committed(
            &mut kernel,
            sample_cell("project:continuitydb:lifecycle-use-direct", 0.95, 10)?,
        )?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: Some(Scope::Project("continuitydb".to_string())),
                valid_at: None,
                system_at: None,
                commit_id: None,
                activation: None,
                lifecycle_stage: None,
                retention_policy: None,
                use_policy: Some(UsePolicy::VerifyBeforeUse),
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
                answerability_question: None,
                compiler_intent: None,
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Execution,
                compiler_policy: ContextCompilerPolicy::RawBaseline,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 20,
            },
        )?;

        assert_eq!(slice.cells, vec![verify]);
        Ok(())
    }

    #[test]
    fn checkout_filters_by_projection_kind() -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let mut procedural = sample_cell("project:continuitydb:projection-procedural", 0.95, 10)?;
        procedural.add_projection(MemoryProjection::new(
            MemoryProjectionKind::Procedural,
            "Follow the validated release procedure.",
            Confidence::new(0.92)?,
            CellCost::new(6, 0)?,
        )?);
        let procedural = append_committed(&mut kernel, procedural)?;
        let mut semantic = sample_cell("project:continuitydb:projection-semantic", 0.95, 10)?;
        semantic.add_projection(MemoryProjection::new(
            MemoryProjectionKind::Semantic,
            "Release artifacts are retained evidence.",
            Confidence::new(0.92)?,
            CellCost::new(6, 0)?,
        )?);
        append_committed(&mut kernel, semantic)?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: Some(Scope::Project("continuitydb".to_string())),
                valid_at: None,
                system_at: None,
                commit_id: None,
                activation: None,
                lifecycle_stage: None,
                retention_policy: None,
                use_policy: None,
                promotion_policy: None,
                projection_kind: Some(MemoryProjectionKind::Procedural),
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
                answerability_question: None,
                compiler_intent: None,
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Execution,
                compiler_policy: ContextCompilerPolicy::RawBaseline,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 20,
            },
        )?;

        assert_eq!(slice.cells, vec![procedural]);
        Ok(())
    }

    #[test]
    fn checkout_filters_by_uncertainty_thresholds() -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let mut surprising = sample_cell("project:continuitydb:uncertainty-surprising", 0.95, 10)?;
        surprising.set_uncertainty(EpistemicUncertainty::new(
            Confidence::new(0.76)?,
            4.2,
            "baseline belief failed",
        )?);
        let surprising = append_committed(&mut kernel, surprising)?;
        let mut routine = sample_cell("project:continuitydb:uncertainty-routine", 0.95, 10)?;
        routine.set_uncertainty(EpistemicUncertainty::new(
            Confidence::new(0.4)?,
            0.8,
            "minor ambiguity",
        )?);
        append_committed(&mut kernel, routine)?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: Some(Scope::Project("continuitydb".to_string())),
                valid_at: None,
                system_at: None,
                commit_id: None,
                activation: None,
                lifecycle_stage: None,
                retention_policy: None,
                use_policy: None,
                promotion_policy: None,
                projection_kind: None,
                minimum_uncertainty: Some(Confidence::new(0.7)?),
                minimum_surprise_bits: Some(3.0),
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
                answerability_question: None,
                compiler_intent: None,
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Execution,
                compiler_policy: ContextCompilerPolicy::RawBaseline,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 20,
            },
        )?;

        assert_eq!(slice.cells, vec![surprising]);
        Ok(())
    }

    #[test]
    fn checkout_filters_by_minimum_probability_delta() -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let mut shifted = sample_cell("project:continuitydb:probability-shift", 0.95, 10)?;
        let expectation = EpistemicExpectation::from_expected_outcome(
            "release artifact exists",
            Confidence::new(0.9)?,
            false,
        )?;
        shifted.set_uncertainty(EpistemicUncertainty::from_expectation(
            Confidence::new(0.7)?,
            expectation,
            "high-certainty baseline failed",
        )?);
        let shifted = append_committed(&mut kernel, shifted)?;
        let mut routine = sample_cell("project:continuitydb:probability-routine", 0.95, 10)?;
        let routine_expectation = EpistemicExpectation::from_expected_outcome(
            "routine check remains ambiguous",
            Confidence::new(0.5)?,
            false,
        )?;
        routine.set_uncertainty(EpistemicUncertainty::from_expectation(
            Confidence::new(0.4)?,
            routine_expectation,
            "low-certainty baseline changed little",
        )?);
        append_committed(&mut kernel, routine)?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: Some(Scope::Project("continuitydb".to_string())),
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
                minimum_probability_delta: Some(0.7),
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
                answerability_question: None,
                compiler_intent: None,
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Execution,
                compiler_policy: ContextCompilerPolicy::RawBaseline,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 20,
            },
        )?;

        assert_eq!(slice.cells, vec![shifted]);
        Ok(())
    }

    #[test]
    fn checkout_filters_by_epistemic_action() -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let mut scavenge = sample_cell("project:continuitydb:action-scavenge", 0.95, 10)?;
        let expectation = EpistemicExpectation::from_expected_outcome(
            "release asset exists",
            Confidence::new(0.9)?,
            false,
        )?;
        scavenge.set_uncertainty(EpistemicUncertainty::from_expectation(
            Confidence::new(0.82)?,
            expectation,
            "strong baseline failed and needs missing evidence",
        )?);
        let scavenge = append_committed(&mut kernel, scavenge)?;
        let use_cell = append_committed(
            &mut kernel,
            sample_cell("project:continuitydb:action-use", 0.95, 10)?,
        )?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: Some(Scope::Project("continuitydb".to_string())),
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
                epistemic_action: Some(EpistemicAction::Scavenge),
                epistemic_action_reason: None,
                selection_reason: None,
                trajectory_memory_strategy: None,
                minimum_trajectory_memory_confidence: None,
                answerability_question: None,
                compiler_intent: None,
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Planning,
                compiler_policy: ContextCompilerPolicy::RawBaseline,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 20,
            },
        )?;

        assert_eq!(slice.cells, vec![scavenge]);
        assert_ne!(slice.cells, vec![use_cell]);
        Ok(())
    }

    #[test]
    fn checkout_filters_by_epistemic_action_reason() -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let mut surprising = sample_cell("project:continuitydb:reason-surprise", 0.95, 10)?;
        let expectation = EpistemicExpectation::from_expected_outcome(
            "release asset exists",
            Confidence::new(0.9)?,
            false,
        )?;
        surprising.set_uncertainty(EpistemicUncertainty::from_expectation(
            Confidence::new(0.22)?,
            expectation,
            "baseline failed but confidence remains low",
        )?);
        let surprising = append_committed(&mut kernel, surprising)?;
        let mut uncertain = sample_cell("project:continuitydb:reason-uncertainty", 0.95, 10)?;
        uncertain.set_uncertainty(EpistemicUncertainty::new(
            Confidence::new(0.82)?,
            0.5,
            "uncertain but not surprising",
        )?);
        let uncertain = append_committed(&mut kernel, uncertain)?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: Some(Scope::Project("continuitydb".to_string())),
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
                epistemic_action_reason: Some(EpistemicActionReason::HighSurprise),
                selection_reason: None,
                trajectory_memory_strategy: None,
                minimum_trajectory_memory_confidence: None,
                answerability_question: None,
                compiler_intent: None,
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Planning,
                compiler_policy: ContextCompilerPolicy::RawBaseline,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 20,
            },
        )?;

        assert_eq!(slice.cells, vec![surprising]);
        assert_ne!(slice.cells, vec![uncertain]);
        Ok(())
    }

    #[test]
    fn checkout_slice_includes_citations_uncertainty_and_frontier_metadata(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let active = sample_cell("project:continuitydb:active", 0.95, 10)?;
        let mut frontier = sample_cell("project:continuitydb:frontier", 0.72, 10)?;
        frontier.activation = ActivationState::Frontier;
        let active = append_committed(&mut kernel, active)?;
        let frontier = append_committed(&mut kernel, frontier)?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: Some(Scope::Project("continuitydb".to_string())),
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
                answerability_question: None,
                compiler_intent: None,
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Execution,
                compiler_policy: ContextCompilerPolicy::RawBaseline,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 20,
            },
        )?;

        assert_eq!(slice.cells, vec![active.clone(), frontier.clone()]);
        assert_eq!(slice.audit_traces[0].cell_id, active.id);
        assert_eq!(
            slice.audit_traces[0].citations,
            vec!["test://project:continuitydb:active".to_string()]
        );
        assert_eq!(slice.audit_traces[1].cell_id, frontier.id);
        assert_eq!(
            slice.audit_traces[1].citations,
            vec!["test://project:continuitydb:frontier".to_string()]
        );
        assert_eq!(slice.uncertainty[0].cell_id, active.id);
        assert_eq!(slice.uncertainty[0].max_confidence, Confidence::new(0.95)?);
        assert_eq!(slice.uncertainty[1].cell_id, frontier.id);
        assert_eq!(slice.uncertainty[1].max_confidence, Confidence::new(0.72)?);
        assert_eq!(slice.frontier_recommendations.len(), 1);
        assert_eq!(slice.frontier_recommendations[0].cell_id, frontier.id);
        assert_eq!(
            slice.frontier_recommendations[0].citations,
            vec!["test://project:continuitydb:frontier".to_string()]
        );
        Ok(())
    }

    #[test]
    fn checkout_uncertainty_preserves_native_statecell_contract(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let mut uncertain = sample_cell("project:continuitydb:native-uncertainty", 0.9, 20)?;
        uncertain.set_uncertainty(EpistemicUncertainty::new(
            Confidence::new(0.73)?,
            4.2,
            "baseline belief failed",
        )?);
        let uncertain = append_committed(&mut kernel, uncertain)?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: Some(Scope::Project("continuitydb".to_string())),
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
                answerability_question: None,
                compiler_intent: None,
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Planning,
                compiler_policy: ContextCompilerPolicy::RawBaseline,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 20,
            },
        )?;

        assert_eq!(slice.cells, vec![uncertain.clone()]);
        assert_eq!(slice.uncertainty[0].cell_id, uncertain.id);
        assert_eq!(slice.uncertainty[0].max_confidence, Confidence::new(0.9)?);
        assert_eq!(
            slice.uncertainty[0].uncertainty_score,
            Confidence::new(0.73)?
        );
        assert_eq!(slice.uncertainty[0].surprise_bits, 4.2);
        assert_eq!(
            slice.uncertainty[0].rationale.as_deref(),
            Some("baseline belief failed")
        );
        assert_eq!(
            slice.context_packets[0].lines,
            vec!["Uncertainty 0.730, surprise 4.200 bits: baseline belief failed".to_string()]
        );
        assert_eq!(
            slice.context_packets[0].entries[0].source,
            ContextPacketEntrySource::NativeUncertainty
        );
        assert_eq!(
            slice.context_packets[0].entries[0].confidence,
            Confidence::new(0.73)?
        );
        Ok(())
    }

    #[test]
    fn checkout_context_packets_preserve_answerability_intent(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let answerable = append_committed(
            &mut kernel,
            sample_cell_with_question_and_source(
                "project:continuitydb:answerability-packet",
                "what should ship?",
                "test://answerability-packet",
                0.93,
                18,
            )?,
        )?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: Some(Scope::Project("continuitydb".to_string())),
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
                answerability_question: Some("what should ship?".to_string()),
                compiler_intent: None,
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Execution,
                compiler_policy: ContextCompilerPolicy::RawBaseline,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 18,
            },
        )?;

        assert_eq!(slice.cells, vec![answerable.clone()]);
        let selection = slice.context_packets[0]
            .selection
            .as_ref()
            .ok_or_else(|| std::io::Error::other("missing packet selection"))?;
        assert_eq!(
            selection.answerability_questions,
            vec!["what should ship?".to_string()]
        );
        assert!(selection
            .reasons
            .contains(&ContextPacketSelectionReason::Answerability));
        Ok(())
    }

    #[test]
    fn checkout_filters_by_selection_reason() -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let mut uncertain = sample_cell(
            "project:continuitydb:selection-reason-native-uncertainty",
            0.93,
            18,
        )?;
        uncertain.set_uncertainty(EpistemicUncertainty::new(
            Confidence::new(0.82)?,
            3.1,
            "high uncertainty should be surfaced",
        )?);
        let uncertain = append_committed(&mut kernel, uncertain)?;
        let routine = append_committed(
            &mut kernel,
            sample_cell(
                "project:continuitydb:selection-reason-evidence-only",
                0.91,
                18,
            )?,
        )?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: Some(Scope::Project("continuitydb".to_string())),
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
                selection_reason: Some(ContextPacketSelectionReason::NativeUncertainty),
                trajectory_memory_strategy: None,
                minimum_trajectory_memory_confidence: None,
                answerability_question: None,
                compiler_intent: None,
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Debugging,
                compiler_policy: ContextCompilerPolicy::RawBaseline,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 1200,
            },
        )?;

        assert_eq!(slice.cells, vec![uncertain]);
        assert!(!slice.cells.contains(&routine));
        let selection = slice.context_packets[0]
            .selection
            .as_ref()
            .ok_or_else(|| std::io::Error::other("missing packet selection"))?;
        assert!(selection
            .reasons
            .contains(&ContextPacketSelectionReason::NativeUncertainty));
        Ok(())
    }

    #[test]
    fn checkout_filters_by_trajectory_memory_reuse_contract(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let mut reusable = sample_cell(
            "project:continuitydb:trajectory-reuse-falsification",
            0.93,
            18,
        )?;
        reusable.set_trajectory_memory(TrajectoryMemory::new(
            "reuse failed release-upload trajectory",
            "identified package artifact and upload command",
            "GitHub release upload targeted a missing release",
            "target/alpha-workflow/release-upload-failure.json",
            0.88,
            "verify release target before trusting upload state",
            vec!["release upload workflow".to_string()],
            vec!["matching successful upload report exists".to_string()],
            ContextPacketStrategy::FalsificationBrief,
        )?);
        let reusable = append_committed(&mut kernel, reusable)?;
        let mut weak = sample_cell("project:continuitydb:trajectory-reuse-weak", 0.93, 18)?;
        weak.set_trajectory_memory(TrajectoryMemory::new(
            "retry upload from stale checkout",
            "found release workflow",
            "failure was not reproduced",
            "target/weak-trace.json",
            0.42,
            "weak lesson should not cross confidence threshold",
            vec!["release upload workflow".to_string()],
            vec!["fresh run contradicts it".to_string()],
            ContextPacketStrategy::FalsificationBrief,
        )?);
        append_committed(&mut kernel, weak)?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: Some(Scope::Project("continuitydb".to_string())),
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
                selection_reason: Some(ContextPacketSelectionReason::TrajectoryMemory),
                trajectory_memory_strategy: Some(ContextPacketStrategy::FalsificationBrief),
                minimum_trajectory_memory_confidence: Some(Confidence::new(0.8)?),
                answerability_question: None,
                compiler_intent: None,
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Planning,
                compiler_policy: ContextCompilerPolicy::RawBaseline,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 1200,
            },
        )?;

        assert_eq!(slice.cells, vec![reusable]);
        assert_eq!(
            slice.context_packets[0].strategy,
            ContextPacketStrategy::FalsificationBrief
        );
        Ok(())
    }

    #[test]
    fn checkout_filters_by_context_gap_kind_and_priority() -> Result<(), Box<dyn std::error::Error>>
    {
        let mut kernel = MemoryKernel::default();
        let routine = append_committed(
            &mut kernel,
            sample_cell(
                "project:continuitydb:checkout-context-gap-routine",
                0.91,
                18,
            )?,
        )?;
        let mut low_priority = sample_cell(
            "project:continuitydb:checkout-context-gap-low-priority",
            0.91,
            18,
        )?;
        low_priority.add_context_gap(ContextGap::new(
            ContextGapKind::MissingEvidence,
            "which artifact proves the claim?",
            "low-priority gap should not cross the checkout threshold",
            0.4,
        )?);
        let low_priority = append_committed(&mut kernel, low_priority)?;
        let mut high_priority = sample_cell(
            "project:continuitydb:checkout-context-gap-high-priority",
            0.91,
            18,
        )?;
        high_priority.add_context_gap(ContextGap::new(
            ContextGapKind::MissingEvidence,
            "which retained artifact proves the live run?",
            "high-priority missing evidence should remain retrievable",
            0.9,
        )?);
        let high_priority = append_committed(&mut kernel, high_priority)?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: Some(Scope::Project("continuitydb".to_string())),
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
                context_gap_kind: Some(ContextGapKind::MissingEvidence),
                minimum_context_gap_priority: Some(Confidence::new(0.7)?),
                invalidation_condition_kind: None,
                minimum_invalidation_priority: None,
                epistemic_action: None,
                epistemic_action_reason: None,
                selection_reason: None,
                trajectory_memory_strategy: None,
                minimum_trajectory_memory_confidence: None,
                answerability_question: None,
                compiler_intent: None,
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Planning,
                compiler_policy: ContextCompilerPolicy::RawBaseline,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 18,
            },
        )?;

        assert_eq!(slice.cells, vec![high_priority.clone()]);
        assert!(!slice.cells.contains(&routine));
        assert!(!slice.cells.contains(&low_priority));
        let selection = slice.context_packets[0]
            .selection
            .as_ref()
            .ok_or_else(|| std::io::Error::other("missing packet selection"))?;
        assert_eq!(selection.context_gaps, high_priority.context_gaps);
        assert!(selection
            .reasons
            .contains(&ContextPacketSelectionReason::ContextGap));
        assert_eq!(slice.summary.context_gap_count, 1);
        assert_eq!(
            slice
                .summary
                .context_gap_kind_counts
                .iter()
                .map(|summary| (summary.kind, summary.count))
                .collect::<Vec<_>>(),
            vec![(ContextGapKind::MissingEvidence, 1)]
        );
        assert_eq!(
            slice.summary.maximum_context_gap_priority,
            Some(Confidence::new(0.9)?)
        );
        Ok(())
    }

    #[test]
    fn checkout_summary_counts_invalidation_conditions() -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let mut dependency_invalidated = sample_cell(
            "project:continuitydb:checkout-invalidation-summary-dependency",
            0.91,
            18,
        )?;
        dependency_invalidated.add_invalidation_condition(InvalidationCondition::new(
            InvalidationConditionKind::DependencyInvalidated,
            "a retained dependency artifact is superseded",
            "dependency invalidation should be summarized for packet consumers",
            0.8,
        )?);
        let mut boundary_violation = sample_cell(
            "project:continuitydb:checkout-invalidation-summary-boundary",
            0.91,
            18,
        )?;
        boundary_violation.add_invalidation_condition(InvalidationCondition::new(
            InvalidationConditionKind::BoundaryViolation,
            "the cell was selected outside its valid use boundary",
            "boundary falsifiers should remain visible in aggregate summaries",
            0.95,
        )?);
        append_committed(&mut kernel, dependency_invalidated)?;
        append_committed(&mut kernel, boundary_violation)?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: Some(Scope::Project("continuitydb".to_string())),
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
                answerability_question: None,
                compiler_intent: None,
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Planning,
                compiler_policy: ContextCompilerPolicy::RawBaseline,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 80,
            },
        )?;

        assert_eq!(slice.summary.invalidation_condition_count, 2);
        assert_eq!(
            slice
                .summary
                .invalidation_condition_kind_counts
                .iter()
                .map(|summary| (summary.kind, summary.count))
                .collect::<Vec<_>>(),
            vec![
                (InvalidationConditionKind::BoundaryViolation, 1),
                (InvalidationConditionKind::DependencyInvalidated, 1)
            ]
        );
        assert_eq!(
            slice.summary.maximum_invalidation_priority,
            Some(Confidence::new(0.95)?)
        );
        Ok(())
    }

    #[test]
    fn checkout_filters_by_invalidation_condition_kind_and_priority(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let routine = append_committed(
            &mut kernel,
            sample_cell(
                "project:continuitydb:checkout-invalidation-routine",
                0.91,
                18,
            )?,
        )?;
        let mut low_priority = sample_cell(
            "project:continuitydb:checkout-invalidation-low-priority",
            0.91,
            18,
        )?;
        low_priority.add_invalidation_condition(InvalidationCondition::new(
            InvalidationConditionKind::DependencyInvalidated,
            "a low-impact dependency is superseded",
            "low-priority falsifier should not cross the checkout threshold",
            0.4,
        )?);
        let low_priority = append_committed(&mut kernel, low_priority)?;
        let mut high_priority = sample_cell(
            "project:continuitydb:checkout-invalidation-high-priority",
            0.91,
            18,
        )?;
        high_priority.add_invalidation_condition(InvalidationCondition::new(
            InvalidationConditionKind::DependencyInvalidated,
            "a retained dependency artifact is superseded",
            "high-priority falsifier should remain retrievable",
            0.9,
        )?);
        let high_priority = append_committed(&mut kernel, high_priority)?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: Some(Scope::Project("continuitydb".to_string())),
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
                invalidation_condition_kind: Some(InvalidationConditionKind::DependencyInvalidated),
                minimum_invalidation_priority: Some(Confidence::new(0.7)?),
                epistemic_action: None,
                epistemic_action_reason: None,
                selection_reason: None,
                trajectory_memory_strategy: None,
                minimum_trajectory_memory_confidence: None,
                answerability_question: None,
                compiler_intent: None,
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Planning,
                compiler_policy: ContextCompilerPolicy::RawBaseline,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 18,
            },
        )?;

        assert_eq!(slice.cells, vec![high_priority.clone()]);
        assert!(!slice.cells.contains(&routine));
        assert!(!slice.cells.contains(&low_priority));
        let selection = slice.context_packets[0]
            .selection
            .as_ref()
            .ok_or_else(|| std::io::Error::other("missing packet selection"))?;
        assert_eq!(
            selection.invalidation_conditions,
            high_priority.invalidation_conditions
        );
        assert!(selection
            .reasons
            .contains(&ContextPacketSelectionReason::InvalidationCondition));
        Ok(())
    }

    #[test]
    fn checkout_slice_materializes_execution_context_packets(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let mut cell = sample_cell("project:continuitydb:projection-checkout", 0.95, 18)?;
        cell.add_projection(MemoryProjection::new(
            MemoryProjectionKind::Episodic,
            "Observed failed release upload at the GitHub asset step.",
            Confidence::new(0.99)?,
            CellCost::new(9, 0)?,
        )?);
        cell.add_projection(MemoryProjection::new(
            MemoryProjectionKind::Semantic,
            "Current belief: release asset exists but upload target may be stale.",
            Confidence::new(0.91)?,
            CellCost::new(8, 0)?,
        )?);
        cell.add_projection(MemoryProjection::new(
            MemoryProjectionKind::Procedural,
            "Next action: verify release id before retrying asset upload.",
            Confidence::new(0.88)?,
            CellCost::new(10, 0)?,
        )?);
        let cell = append_committed(&mut kernel, cell)?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: Some(Scope::Project("continuitydb".to_string())),
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
                answerability_question: None,
                compiler_intent: None,
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Execution,
                compiler_policy: ContextCompilerPolicy::RawBaseline,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 18,
            },
        )?;

        assert_eq!(slice.cells, vec![cell.clone()]);
        assert_eq!(slice.context_packets.len(), 1);
        assert_eq!(slice.context_packets[0].profile, ContextProfile::Execution);
        assert_eq!(
            slice.context_packets[0].strategy,
            ContextPacketStrategy::OperationalBrief
        );
        assert_eq!(
            slice.context_packets[0].origin.as_ref().map(|origin| (
                origin.cell_id,
                origin.lifecycle_stage,
                origin.activation
            )),
            Some((cell.id, cell.lifecycle_stage, cell.activation))
        );
        assert_eq!(
            slice.context_packets[0]
                .selection
                .as_ref()
                .map(|selection| (
                    selection.max_confidence,
                    selection.utility_score,
                    selection.uncertainty_score,
                    selection.surprise_bits
                )),
            Some((Confidence::new(0.95)?, 0.5, Confidence::new(0.0)?, 0.0))
        );
        assert_eq!(slice.context_packets[0].token_count, 18);
        assert_eq!(
            slice.context_packets[0].lines,
            vec![
                "Current belief: release asset exists but upload target may be stale.".to_string(),
                "Next action: verify release id before retrying asset upload.".to_string(),
            ]
        );
        assert_eq!(
            slice.context_packets[0]
                .entries
                .iter()
                .map(|entry| (&entry.source, entry.confidence, entry.token_count))
                .collect::<Vec<_>>(),
            vec![
                (
                    &ContextPacketEntrySource::Projection(MemoryProjectionKind::Semantic),
                    Confidence::new(0.91)?,
                    8
                ),
                (
                    &ContextPacketEntrySource::Projection(MemoryProjectionKind::Procedural),
                    Confidence::new(0.88)?,
                    10
                )
            ]
        );
        assert_eq!(slice.summary.context_packet_count, 1);
        Ok(())
    }

    #[test]
    fn checkout_context_compiler_emits_revision_uncertainty_capsule(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let committed_at = test_commit_time()?;
        let old = append_committed(
            &mut kernel,
            sample_cell("project:continuitydb:release-asset-belief-old", 0.95, 8)?,
        )?;
        let mut current = sample_cell(
            "project:continuitydb:release-asset-belief-current",
            0.91,
            18,
        )?;
        current.add_projection(MemoryProjection::new(
            MemoryProjectionKind::Semantic,
            "Current belief: release upload failed because the GitHub release target was wrong.",
            Confidence::new(0.91)?,
            CellCost::new(10, 0)?,
        )?);
        let expectation = EpistemicExpectation::from_expected_outcome(
            "release asset upload exists",
            Confidence::new(0.9)?,
            false,
        )?;
        current.set_uncertainty(EpistemicUncertainty::from_expectation(
            Confidence::new(0.72)?,
            expectation,
            "prior release-readiness belief failed",
        )?);
        current.set_calibration(EpistemicCalibration::new(
            Confidence::new(0.92)?,
            Confidence::new(0.55)?,
            30,
            "release automation claims have been overconfident",
        )?);
        current.add_context_gap(ContextGap::new(
            ContextGapKind::MissingEvidence,
            "which retained upload artifact proves the failure?",
            "avoid collapsing release state without retained evidence",
            0.8,
        )?);
        current.add_invalidation_condition(InvalidationCondition::new(
            InvalidationConditionKind::ContradictoryEvidence,
            "a retained successful upload report with matching digest exists",
            "successful upload evidence would supersede the failure belief",
            0.9,
        )?);
        let current = append_committed(&mut kernel, current)?;
        kernel.append_revision_link(RevisionLinkRecord::new(
            current.id,
            RevisionLinkKind::Supersedes,
            old.id,
            committed_at,
        ))?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: Some(SemanticAnchor::new(
                    "project:continuitydb:release-asset-belief-current",
                )),
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
                answerability_question: None,
                compiler_intent: None,
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Planning,
                compiler_policy: ContextCompilerPolicy::RawBaseline,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 40,
            },
        )?;

        let packet = &slice.context_packets[0];
        assert_eq!(packet.strategy, ContextPacketStrategy::RevisionCapsule);
        assert!(packet.token_count <= 40);
        assert!(packet
            .revision_context
            .iter()
            .any(|context| context.related_cell_id == old.id));
        assert!(packet
            .lines
            .iter()
            .any(|line| line.contains("Current belief: release upload failed")));
        assert!(packet
            .lines
            .iter()
            .any(|line| line.contains("Supersedes prior belief")));
        assert!(packet
            .lines
            .iter()
            .any(|line| line.contains("Calibration warning")));
        assert!(packet
            .lines
            .iter()
            .any(|line| line.contains("Missing context")));
        assert!(packet
            .lines
            .iter()
            .any(|line| line.contains("Invalidation condition")));
        assert_eq!(slice.summary.revision_context_count, 1);
        assert_eq!(slice.summary.context_gap_count, 1);
        assert_eq!(slice.summary.invalidation_condition_count, 1);
        Ok(())
    }

    #[test]
    fn checkout_context_compiler_honors_trajectory_memory_strategy(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let mut cell = sample_cell("project:continuitydb:trajectory-reuse", 0.93, 16)?;
        cell.set_trajectory_memory(TrajectoryMemory::new(
            "retry release upload after workflow hardening",
            "release preflight and upload reports were retained",
            "upload failed because the target GitHub Release was missing",
            "artifact://rollout/release-upload-404",
            0.86,
            "ensure the release exists before uploading retained assets",
            vec!["publishing release assets from CI".to_string()],
            vec!["release lookup succeeds and upload report validates".to_string()],
            ContextPacketStrategy::ScavengingBrief,
        )?);
        let cell = append_committed(&mut kernel, cell)?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: Some(Scope::Project("continuitydb".to_string())),
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
                answerability_question: None,
                compiler_intent: None,
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Reflection,
                compiler_policy: ContextCompilerPolicy::RawBaseline,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 20,
            },
        )?;

        assert_eq!(slice.cells, vec![cell]);
        assert_eq!(slice.context_packets.len(), 1);
        assert_eq!(
            slice.context_packets[0].strategy,
            ContextPacketStrategy::ScavengingBrief
        );
        assert_eq!(
            slice.context_packets[0].abstraction_level,
            ContextAbstractionLevel::Scavenging
        );
        assert!(slice.context_packets[0]
            .selection
            .as_ref()
            .is_some_and(|selection| selection
                .reasons
                .contains(&ContextPacketSelectionReason::TrajectoryMemory)));
        assert!(slice.context_packets[0].lines.iter().any(
            |line| line.contains("ensure the release exists before uploading retained assets")
        ));
        Ok(())
    }

    #[test]
    fn checkout_task_intent_prefers_applicable_trajectory_memory(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let mut applicable = sample_cell("project:continuitydb:release-upload-reuse", 0.84, 16)?;
        applicable.set_trajectory_memory(TrajectoryMemory::new(
            "rerun release upload without checking release existence",
            "release preflight retained enough evidence to diagnose the upload failure",
            "asset upload returned 404 because the GitHub Release did not exist",
            "artifact://rollout/release-upload-404",
            0.91,
            "before retrying release upload, verify the GitHub Release exists",
            vec!["retrying release upload from CI".to_string()],
            vec!["release existence has already been verified".to_string()],
            ContextPacketStrategy::FalsificationBrief,
        )?);
        let applicable = append_committed(&mut kernel, applicable)?;
        let mut irrelevant = sample_cell("project:continuitydb:migration-reuse", 0.98, 16)?;
        irrelevant.set_trajectory_memory(TrajectoryMemory::new(
            "rerun database migration after schema drift",
            "migration dry-run identified a missing index",
            "migration failed because the database schema was stale",
            "artifact://rollout/database-migration",
            0.99,
            "before retrying migration, verify schema index readiness",
            vec!["retrying database migration".to_string()],
            vec!["schema index readiness has already been verified".to_string()],
            ContextPacketStrategy::ScavengingBrief,
        )?);
        append_committed(&mut kernel, irrelevant)?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: Some(Scope::Project("continuitydb".to_string())),
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
                selection_reason: Some(ContextPacketSelectionReason::TrajectoryMemory),
                trajectory_memory_strategy: Some(ContextPacketStrategy::FalsificationBrief),
                minimum_trajectory_memory_confidence: Some(Confidence::new(0.8)?),
                answerability_question: None,
                compiler_intent: Some(
                    "retry release upload without repeating the GitHub Release 404".to_string(),
                ),
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Reflection,
                compiler_policy: ContextCompilerPolicy::Automatic,
                minimum_confidence: Confidence::new(0.8)?,
                token_budget: 20,
            },
        )?;

        assert_eq!(slice.cells, vec![applicable]);
        assert_eq!(
            slice.context_packets[0].strategy,
            ContextPacketStrategy::FalsificationBrief
        );
        assert!(slice.context_packets[0]
            .compiler_reason_tags
            .contains(&"trajectory-applicability-match".to_string()));
        assert!(slice.context_packets[0]
            .lines
            .iter()
            .any(|line| line.contains("before retrying release upload")));
        Ok(())
    }

    #[test]
    fn checkout_task_intent_suppresses_invalidated_trajectory_memory(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let mut invalidated = sample_cell("project:continuitydb:invalidated-reuse", 0.84, 16)?;
        invalidated.set_trajectory_memory(TrajectoryMemory::new(
            "rerun release upload without checking release existence",
            "release preflight retained enough evidence to diagnose the upload failure",
            "asset upload returned 404 because the GitHub Release did not exist",
            "artifact://rollout/release-upload-invalidated",
            0.95,
            "before retrying release upload, verify the GitHub Release exists",
            vec!["retrying release upload from CI".to_string()],
            vec!["release existence has already been verified".to_string()],
            ContextPacketStrategy::FalsificationBrief,
        )?);
        append_committed(&mut kernel, invalidated)?;
        let fallback = append_committed(
            &mut kernel,
            sample_cell("project:continuitydb:fallback-current-state", 0.96, 16)?,
        )?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: Some(Scope::Project("continuitydb".to_string())),
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
                answerability_question: None,
                compiler_intent: Some(
                    "retry release upload after release existence has already been verified"
                        .to_string(),
                ),
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Reflection,
                compiler_policy: ContextCompilerPolicy::Automatic,
                minimum_confidence: Confidence::new(0.8)?,
                token_budget: 16,
            },
        )?;

        assert_eq!(slice.cells, vec![fallback]);
        assert!(!slice.context_packets[0]
            .compiler_reason_tags
            .contains(&"trajectory-applicability-match".to_string()));
        Ok(())
    }

    #[test]
    fn checkout_context_compiler_accepts_raw_baseline_policy(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let cell = append_committed(
            &mut kernel,
            sample_cell("project:continuitydb:baseline-policy-default", 0.95, 8)?,
        )?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: Some(cell.anchors[0].clone()),
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
                answerability_question: None,
                compiler_intent: None,
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Execution,
                compiler_policy: ContextCompilerPolicy::RawBaseline,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 12,
            },
        )?;

        assert_eq!(
            slice.context_packets[0].compiler_policy,
            ContextCompilerPolicy::RawBaseline
        );
        Ok(())
    }

    #[test]
    fn checkout_model_assisted_compiler_requires_accepted_proposal(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let cell = append_committed(
            &mut kernel,
            sample_cell(
                "project:continuitydb:model-assisted-requires-contract",
                0.95,
                8,
            )?,
        )?;

        let result = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: Some(cell.anchors[0].clone()),
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
                answerability_question: None,
                compiler_intent: None,
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Execution,
                compiler_policy: ContextCompilerPolicy::ModelAssisted,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 12,
            },
        );

        assert_eq!(result, Err(CheckoutError::MissingContextCompilerProposal));
        Ok(())
    }

    #[test]
    fn checkout_model_assisted_compiler_applies_accepted_proposal(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let mut cell = sample_cell(
            "project:continuitydb:model-assisted-applies-contract",
            0.95,
            8,
        )?;
        cell.add_context_gap(ContextGap::new(
            ContextGapKind::MissingEvidence,
            "which retained artifact proves model-shaped packet choice?",
            "model-assisted packet shaping must stay evidence-backed",
            0.8,
        )?);
        let cell = append_committed(&mut kernel, cell)?;
        let proposal = ContextCompilerProposal::new(
            cell.id,
            ContextPacketStrategy::ScavengingBrief,
            ContextAbstractionLevel::Scavenging,
            vec!["model-assisted-scavenging".to_string()],
            vec![format!("test://{}", cell.anchors[0].as_str())],
        )?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: Some(cell.anchors[0].clone()),
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
                answerability_question: None,
                compiler_intent: None,
                compiler_proposals: vec![proposal],
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Execution,
                compiler_policy: ContextCompilerPolicy::ModelAssisted,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 24,
            },
        )?;

        let packet = &slice.context_packets[0];
        assert_eq!(packet.compiler_policy, ContextCompilerPolicy::ModelAssisted);
        assert_eq!(packet.strategy, ContextPacketStrategy::ScavengingBrief);
        assert_eq!(
            packet.abstraction_level,
            ContextAbstractionLevel::Scavenging
        );
        assert!(packet
            .compiler_reason_tags
            .contains(&"model-assisted-scavenging".to_string()));
        assert!(packet
            .compiler_reason_tags
            .contains(&"packet-plan-stage:model-assisted".to_string()));
        assert_eq!(
            packet.compiler_evidence_locators,
            vec![format!("test://{}", cell.anchors[0].as_str())]
        );
        assert!(packet
            .lines
            .iter()
            .any(|line| line.contains("Missing context")));
        Ok(())
    }

    #[test]
    fn checkout_model_assisted_compiler_materializes_supported_proposed_lines(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let cell = append_committed(
            &mut kernel,
            sample_cell(
                "project:continuitydb:model-assisted-supported-line",
                0.95,
                8,
            )?,
        )?;
        let proposal = ContextCompilerProposal::new(
            cell.id,
            ContextPacketStrategy::FalsificationBrief,
            ContextAbstractionLevel::Falsification,
            vec!["model-assisted-falsification".to_string()],
            vec!["artifact://compiler/proposal/supported-line".to_string()],
        )?
        .with_proposed_lines(vec![ContextCompilerProposalLine::new(
            "Do not retry the upload until the retained release target is checked.",
            vec![format!("test://{}", cell.anchors[0].as_str())],
            7,
        )?]);

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: Some(cell.anchors[0].clone()),
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
                answerability_question: None,
                compiler_intent: None,
                compiler_proposals: vec![proposal],
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Execution,
                compiler_policy: ContextCompilerPolicy::ModelAssisted,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 24,
            },
        )?;

        assert!(slice.context_packets[0].lines.contains(
            &"Do not retry the upload until the retained release target is checked.".to_string()
        ));
        Ok(())
    }

    #[test]
    fn checkout_model_assisted_operational_brief_materializes_supported_proposed_lines(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let cell = append_committed(
            &mut kernel,
            sample_cell(
                "project:continuitydb:model-assisted-operational-line",
                0.95,
                8,
            )?,
        )?;
        let proposal = ContextCompilerProposal::new(
            cell.id,
            ContextPacketStrategy::OperationalBrief,
            ContextAbstractionLevel::Brief,
            vec!["model-assisted-operational".to_string()],
            vec![format!("test://{}", cell.anchors[0].as_str())],
        )?
        .with_proposed_lines(vec![ContextCompilerProposalLine::new(
            "Use this current operational fact before taking the next step.",
            vec![format!("test://{}", cell.anchors[0].as_str())],
            7,
        )?]);

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: Some(cell.anchors[0].clone()),
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
                answerability_question: None,
                compiler_intent: None,
                compiler_proposals: vec![proposal],
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Execution,
                compiler_policy: ContextCompilerPolicy::ModelAssisted,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 24,
            },
        )?;

        assert!(slice.context_packets[0].lines.contains(
            &"Use this current operational fact before taking the next step.".to_string()
        ));
        assert!(slice.context_packets[0].entries.iter().any(|entry| {
            entry.text == "Use this current operational fact before taking the next step."
                && entry.source == ContextPacketEntrySource::ModelAssistedCompilerLine
                && entry.token_count == 7
        }));
        Ok(())
    }

    #[test]
    fn checkout_model_assisted_falsification_brief_reserves_budget_for_safety_guidance(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let mut cell = sample_cell("project:continuitydb:model-assisted-safety-budget", 0.95, 8)?;
        cell.add_projection(MemoryProjection::new(
            MemoryProjectionKind::Semantic,
            "Current belief: release upload failed because the target release was missing.",
            Confidence::new(0.91)?,
            CellCost::new(10, 0)?,
        )?);
        cell.add_invalidation_condition(InvalidationCondition::new(
            InvalidationConditionKind::ContradictoryEvidence,
            "a retained successful upload report with matching digest exists",
            "successful upload evidence would supersede the failure belief",
            0.9,
        )?);
        let cell = append_committed(&mut kernel, cell)?;
        let proposal = ContextCompilerProposal::new(
            cell.id,
            ContextPacketStrategy::FalsificationBrief,
            ContextAbstractionLevel::Falsification,
            vec!["model-assisted-falsification".to_string()],
            vec![format!("test://{}", cell.anchors[0].as_str())],
        )?
        .with_proposed_lines(vec![ContextCompilerProposalLine::new(
            "Model proposal: retry only after checking the retained release target.",
            vec![format!("test://{}", cell.anchors[0].as_str())],
            7,
        )?]);

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: Some(cell.anchors[0].clone()),
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
                answerability_question: None,
                compiler_intent: None,
                compiler_proposals: vec![proposal],
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Execution,
                compiler_policy: ContextCompilerPolicy::ModelAssisted,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 8,
            },
        )?;

        assert!(slice.context_packets[0]
            .lines
            .iter()
            .any(|line| line.contains("Invalidation condition")));
        assert!(!slice.context_packets[0].lines.contains(
            &"Model proposal: retry only after checking the retained release target.".to_string()
        ));
        Ok(())
    }

    #[test]
    fn checkout_model_assisted_compiler_materializes_trajectory_trace_supported_lines(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let mut cell = sample_cell(
            "project:continuitydb:model-assisted-trajectory-line",
            0.95,
            8,
        )?;
        cell.set_trajectory_memory(TrajectoryMemory::new(
            "retry upload against assumed release target",
            "built and retained the release asset bundle",
            "GitHub upload returned 404 for the target release",
            "artifact://rollout/release-upload-404",
            0.88,
            "verify the release target before retrying asset upload",
            vec!["task is retrying release asset upload".to_string()],
            vec!["release target has been independently verified".to_string()],
            ContextPacketStrategy::FalsificationBrief,
        )?);
        let cell = append_committed(&mut kernel, cell)?;
        let proposal = ContextCompilerProposal::new(
            cell.id,
            ContextPacketStrategy::FalsificationBrief,
            ContextAbstractionLevel::Falsification,
            vec!["model-assisted-trajectory-reuse".to_string()],
            vec!["artifact://rollout/release-upload-404".to_string()],
        )?
        .with_proposed_lines(vec![ContextCompilerProposalLine::new(
            "Trajectory lesson: verify the release target before retrying upload.",
            vec!["artifact://rollout/release-upload-404".to_string()],
            7,
        )?]);

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: Some(cell.anchors[0].clone()),
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
                answerability_question: None,
                compiler_intent: None,
                compiler_proposals: vec![proposal],
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Execution,
                compiler_policy: ContextCompilerPolicy::ModelAssisted,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 28,
            },
        )?;

        let packet = &slice.context_packets[0];
        assert!(packet
            .citations
            .contains(&"artifact://rollout/release-upload-404".to_string()));
        assert_eq!(
            packet.compiler_evidence_locators,
            vec!["artifact://rollout/release-upload-404".to_string()]
        );
        assert!(packet.lines.contains(
            &"Trajectory lesson: verify the release target before retrying upload.".to_string()
        ));
        Ok(())
    }

    #[test]
    fn checkout_model_assisted_compiler_drops_unsupported_proposed_lines(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let cell = append_committed(
            &mut kernel,
            sample_cell(
                "project:continuitydb:model-assisted-unsupported-line",
                0.95,
                8,
            )?,
        )?;
        let proposal = ContextCompilerProposal::new(
            cell.id,
            ContextPacketStrategy::FalsificationBrief,
            ContextAbstractionLevel::Falsification,
            vec!["model-assisted-falsification".to_string()],
            vec!["artifact://compiler/proposal/unsupported-line".to_string()],
        )?
        .with_proposed_lines(vec![ContextCompilerProposalLine::new(
            "Unsupported model-generated instruction must not enter checkout.",
            vec!["artifact://not/source-cell-evidence".to_string()],
            7,
        )?]);

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: Some(cell.anchors[0].clone()),
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
                answerability_question: None,
                compiler_intent: None,
                compiler_proposals: vec![proposal],
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Execution,
                compiler_policy: ContextCompilerPolicy::ModelAssisted,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 24,
            },
        )?;

        assert!(!slice.context_packets[0].lines.contains(
            &"Unsupported model-generated instruction must not enter checkout.".to_string()
        ));
        Ok(())
    }

    #[test]
    fn checkout_model_assisted_compiler_filters_unsupported_evidence_metadata(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let cell = append_committed(
            &mut kernel,
            sample_cell(
                "project:continuitydb:model-assisted-evidence-filter",
                0.95,
                8,
            )?,
        )?;
        let supported_locator = format!("test://{}", cell.anchors[0].as_str());
        let proposal = ContextCompilerProposal::new(
            cell.id,
            ContextPacketStrategy::FalsificationBrief,
            ContextAbstractionLevel::Falsification,
            vec!["model-assisted-falsification".to_string()],
            vec![
                supported_locator.clone(),
                "artifact://not/source-cell-evidence".to_string(),
            ],
        )?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: Some(cell.anchors[0].clone()),
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
                answerability_question: None,
                compiler_intent: None,
                compiler_proposals: vec![proposal],
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Execution,
                compiler_policy: ContextCompilerPolicy::ModelAssisted,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 24,
            },
        )?;

        assert_eq!(
            slice.context_packets[0].compiler_evidence_locators,
            vec![supported_locator]
        );
        Ok(())
    }

    #[test]
    fn checkout_model_assisted_compiler_rejects_duplicate_target_proposals(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let cell = append_committed(
            &mut kernel,
            sample_cell(
                "project:continuitydb:model-assisted-duplicate-contract",
                0.95,
                8,
            )?,
        )?;
        let first = ContextCompilerProposal::new(
            cell.id,
            ContextPacketStrategy::OperationalBrief,
            ContextAbstractionLevel::Brief,
            vec!["model-assisted-operational".to_string()],
            vec!["artifact://compiler/proposal/first".to_string()],
        )?;
        let second = ContextCompilerProposal::new(
            cell.id,
            ContextPacketStrategy::FalsificationBrief,
            ContextAbstractionLevel::Falsification,
            vec!["model-assisted-falsification".to_string()],
            vec!["artifact://compiler/proposal/second".to_string()],
        )?;

        let result = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: Some(cell.anchors[0].clone()),
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
                answerability_question: None,
                compiler_intent: None,
                compiler_proposals: vec![first, second],
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Execution,
                compiler_policy: ContextCompilerPolicy::ModelAssisted,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 24,
            },
        );

        assert_eq!(result, Err(CheckoutError::AmbiguousContextCompilerProposal));
        Ok(())
    }

    #[test]
    fn checkout_automatic_context_compiler_prioritizes_safety_guidance(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let committed_at = test_commit_time()?;
        let old = append_committed(
            &mut kernel,
            sample_cell("project:continuitydb:adaptive-release-belief-old", 0.95, 8)?,
        )?;
        let mut current = sample_cell_with_question_and_source(
            "project:continuitydb:adaptive-release-belief-current",
            "what should I do next without repeating the release upload failure?",
            "test",
            0.91,
            18,
        )?;
        current.add_projection(MemoryProjection::new(
            MemoryProjectionKind::Semantic,
            "Current belief: release upload failed because the GitHub release target was wrong.",
            Confidence::new(0.91)?,
            CellCost::new(10, 0)?,
        )?);
        let expectation = EpistemicExpectation::from_expected_outcome(
            "release asset upload exists",
            Confidence::new(0.9)?,
            false,
        )?;
        current.set_uncertainty(EpistemicUncertainty::from_expectation(
            Confidence::new(0.72)?,
            expectation,
            "prior release-readiness belief failed",
        )?);
        current.set_calibration(EpistemicCalibration::new(
            Confidence::new(0.92)?,
            Confidence::new(0.55)?,
            30,
            "release automation claims have been overconfident",
        )?);
        current.add_context_gap(ContextGap::new(
            ContextGapKind::MissingEvidence,
            "which retained upload artifact proves the failure?",
            "avoid collapsing release state without retained evidence",
            0.8,
        )?);
        current.add_invalidation_condition(InvalidationCondition::new(
            InvalidationConditionKind::ContradictoryEvidence,
            "a retained successful upload report with matching digest exists",
            "successful upload evidence would supersede the failure belief",
            0.9,
        )?);
        let current = append_committed(&mut kernel, current)?;
        kernel.append_revision_link(RevisionLinkRecord::new(
            current.id,
            RevisionLinkKind::Supersedes,
            old.id,
            committed_at,
        ))?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: Some(SemanticAnchor::new(
                    "project:continuitydb:adaptive-release-belief-current",
                )),
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
                answerability_question: Some(
                    "what should I do next without repeating the release upload failure?"
                        .to_string(),
                ),
                compiler_intent: None,
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Planning,
                compiler_policy: ContextCompilerPolicy::Automatic,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 32,
            },
        )?;

        let packet = &slice.context_packets[0];
        assert_eq!(packet.compiler_policy, ContextCompilerPolicy::Automatic);
        assert_eq!(
            packet.abstraction_level,
            ContextAbstractionLevel::Falsification
        );
        assert!(packet.token_count <= 32);
        assert!(packet
            .lines
            .iter()
            .any(|line| line.contains("Current belief: release upload failed")));
        assert!(packet
            .lines
            .iter()
            .any(|line| line.contains("Invalidation condition")));
        assert!(packet
            .lines
            .iter()
            .any(|line| line.contains("Missing context")));
        assert!(packet
            .revision_context
            .iter()
            .any(|context| context.related_cell_id == old.id));
        assert!(packet.origin.is_some());
        assert!(packet.selection.is_some());
        Ok(())
    }

    #[test]
    fn checkout_automatic_context_compiler_uses_intent_separate_from_answerability_filter(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let committed_at = test_commit_time()?;
        let old = append_committed(
            &mut kernel,
            sample_cell("project:continuitydb:adaptive-intent-old", 0.94, 8)?,
        )?;
        let mut current = sample_cell_with_question_and_source(
            "project:continuitydb:adaptive-intent-current",
            "what is the artifact state?",
            "test",
            0.91,
            18,
        )?;
        current.add_projection(MemoryProjection::new(
            MemoryProjectionKind::Semantic,
            "Current belief: release upload failed because the target release was missing.",
            Confidence::new(0.91)?,
            CellCost::new(10, 0)?,
        )?);
        current.add_context_gap(ContextGap::new(
            ContextGapKind::MissingEvidence,
            "which retained upload artifact proves the failed target?",
            "avoid answering from release status alone",
            0.8,
        )?);
        current.add_invalidation_condition(InvalidationCondition::new(
            InvalidationConditionKind::ContradictoryEvidence,
            "a retained successful upload report with matching digest exists",
            "successful upload evidence would change the action",
            0.9,
        )?);
        let current = append_committed(&mut kernel, current)?;
        kernel.append_revision_link(RevisionLinkRecord::new(
            current.id,
            RevisionLinkKind::Supersedes,
            old.id,
            committed_at,
        ))?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: Some(SemanticAnchor::new(
                    "project:continuitydb:adaptive-intent-current",
                )),
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
                answerability_question: Some("what is the artifact state?".to_string()),
                compiler_intent: Some(
                    "what should I do next without repeating the release upload failure?"
                        .to_string(),
                ),
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Planning,
                compiler_policy: ContextCompilerPolicy::Automatic,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 32,
            },
        )?;

        assert_eq!(slice.cells.len(), 1);
        let packet = &slice.context_packets[0];
        assert_eq!(
            packet.abstraction_level,
            ContextAbstractionLevel::Falsification
        );
        assert!(packet
            .lines
            .iter()
            .any(|line| line.contains("Invalidation condition")));
        assert!(packet
            .revision_context
            .iter()
            .any(|context| context.related_cell_id == old.id));
        Ok(())
    }

    #[test]
    fn checkout_automatic_context_compiler_prioritizes_lifecycle_safe_use(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let mut cell = sample_cell("project:continuitydb:lifecycle-safe-use", 0.91, 18)?;
        cell.set_lifecycle_policy(ContextLifecyclePolicy {
            retention: RetentionPolicy::DecayUnlessReinforced,
            use_policy: UsePolicy::DoNotUseForAnswer,
            promotion: PromotionPolicy::Manual,
        });
        cell.add_projection(MemoryProjection::new(
            MemoryProjectionKind::Semantic,
            "Current claim: release deployment completed.",
            Confidence::new(0.91)?,
            CellCost::new(8, 0)?,
        )?);
        append_committed(&mut kernel, cell)?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: Some(SemanticAnchor::new(
                    "project:continuitydb:lifecycle-safe-use",
                )),
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
                answerability_question: None,
                compiler_intent: Some(
                    "what should I do next for the release deployment?".to_string(),
                ),
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Planning,
                compiler_policy: ContextCompilerPolicy::Automatic,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 18,
            },
        )?;

        let packet = &slice.context_packets[0];
        assert_eq!(packet.compiler_policy, ContextCompilerPolicy::Automatic);
        assert_eq!(
            packet.abstraction_level,
            ContextAbstractionLevel::Falsification
        );
        assert!(packet
            .compiler_reason_tags
            .contains(&"lifecycle-safe-use-policy".to_string()));
        assert_eq!(
            packet.lines.first().map(String::as_str),
            Some("Lifecycle policy: decay unless reinforced; do not use for answer; manual promotion")
        );
        assert!(packet.token_count <= 18);
        Ok(())
    }

    #[test]
    fn checkout_automatic_context_compiler_uses_affordance_risk_without_keyword_intent(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let mut cell = sample_cell("project:continuitydb:affordance-risk", 0.88, 12)?;
        cell.set_context_affordance(ContextAffordance::new(0.80, 0.85, 0.95, 0.90, 0.75, 0.10)?);
        cell.add_projection(MemoryProjection::new(
            MemoryProjectionKind::Semantic,
            "Current claim: migration state is probably complete.",
            Confidence::new(0.88)?,
            CellCost::new(10, 0)?,
        )?);
        append_committed(&mut kernel, cell)?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: Some(SemanticAnchor::new("project:continuitydb:affordance-risk")),
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
                answerability_question: None,
                compiler_intent: None,
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Planning,
                compiler_policy: ContextCompilerPolicy::Automatic,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 12,
            },
        )?;

        let packet = &slice.context_packets[0];
        assert_eq!(packet.compiler_policy, ContextCompilerPolicy::Automatic);
        assert_eq!(
            packet.abstraction_level,
            ContextAbstractionLevel::EvidenceDense
        );
        assert!(packet
            .compiler_reason_tags
            .contains(&"context-affordance-risk".to_string()));
        assert!(packet
            .lines
            .first()
            .is_some_and(|line| line.starts_with("Context affordance")));
        assert!(packet.token_count <= 12);
        Ok(())
    }

    #[test]
    fn checkout_packet_planner_marks_lifecycle_hard_gate_under_token_pressure(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let mut cell = sample_cell("project:continuitydb:packet-plan-lifecycle", 0.95, 12)?;
        cell.set_lifecycle_policy(ContextLifecyclePolicy {
            retention: RetentionPolicy::Persistent,
            use_policy: UsePolicy::DoNotUseForAnswer,
            promotion: PromotionPolicy::Manual,
        });
        cell.add_projection(MemoryProjection::new(
            MemoryProjectionKind::Semantic,
            "Do not let this projection crowd out lifecycle safety.",
            Confidence::new(0.95)?,
            CellCost::new(8, 0)?,
        )?);
        append_committed(&mut kernel, cell)?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: Some(SemanticAnchor::new(
                    "project:continuitydb:packet-plan-lifecycle",
                )),
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
                answerability_question: None,
                compiler_intent: Some("give the direct release action".to_string()),
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Execution,
                compiler_policy: ContextCompilerPolicy::Automatic,
                minimum_confidence: Confidence::new(0.8)?,
                token_budget: 12,
            },
        )?;

        let packet = &slice.context_packets[0];
        assert_eq!(packet.strategy, ContextPacketStrategy::FalsificationBrief);
        assert!(packet
            .compiler_reason_tags
            .contains(&"packet-plan-hard-gate:lifecycle-policy".to_string()));
        assert_eq!(
            packet.lines.first().map(String::as_str),
            Some("Lifecycle policy: persistent retention; do not use for answer; manual promotion")
        );
        assert_eq!(
            packet.plan.as_ref().map(|plan| plan.purpose),
            Some(ContextPacketPurpose::SafetyGuidance)
        );
        assert!(packet.plan.as_ref().is_some_and(|plan| plan
            .required_contracts
            .contains(&ContextPacketRequirement::LifecyclePolicy)));
        assert!(packet.token_count <= 12);
        Ok(())
    }

    #[test]
    fn checkout_packet_planner_changes_compilation_without_changing_retrieval(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let mut cell = sample_cell("project:continuitydb:packet-plan-same-retrieval", 0.92, 12)?;
        cell.add_invalidation_condition(InvalidationCondition::new(
            InvalidationConditionKind::ContradictoryEvidence,
            "a retained successful upload report exists",
            "success evidence would change the action",
            0.9,
        )?);
        append_committed(&mut kernel, cell)?;

        let baseline = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: Some(SemanticAnchor::new(
                    "project:continuitydb:packet-plan-same-retrieval",
                )),
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
                answerability_question: None,
                compiler_intent: Some("what should I do next?".to_string()),
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Planning,
                compiler_policy: ContextCompilerPolicy::RawBaseline,
                minimum_confidence: Confidence::new(0.8)?,
                token_budget: 12,
            },
        )?;
        let adaptive = checkout(
            &kernel,
            CheckoutRequest {
                compiler_policy: ContextCompilerPolicy::Automatic,
                ..CheckoutRequest {
                    semantic_anchor: Some(SemanticAnchor::new(
                        "project:continuitydb:packet-plan-same-retrieval",
                    )),
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
                    answerability_question: None,
                    compiler_intent: Some("what should I do next?".to_string()),
                    compiler_proposals: Vec::new(),
                    evidence_source: None,
                    dependency_target: None,
                    dependency_kind: None,
                    revision_related_cell: None,
                    revision_link_kind: None,
                    context_profile: ContextProfile::Planning,
                    compiler_policy: ContextCompilerPolicy::RawBaseline,
                    minimum_confidence: Confidence::new(0.8)?,
                    token_budget: 12,
                }
            },
        )?;

        assert_eq!(baseline.cells, adaptive.cells);
        assert_eq!(baseline.alternatives, adaptive.alternatives);
        assert!(!baseline.context_packets[0]
            .compiler_reason_tags
            .contains(&"packet-plan-stage:automatic".to_string()));
        assert!(adaptive.context_packets[0]
            .compiler_reason_tags
            .contains(&"packet-plan-stage:automatic".to_string()));
        assert!(adaptive.context_packets[0]
            .compiler_reason_tags
            .contains(&"packet-plan-hard-gate:invalidation".to_string()));
        assert_eq!(
            adaptive.context_packets[0]
                .plan
                .as_ref()
                .map(|plan| plan.purpose),
            Some(ContextPacketPurpose::Falsification)
        );
        Ok(())
    }

    #[test]
    fn checkout_packet_planner_suppresses_sole_invalidated_trajectory_line(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let mut cell = sample_cell("project:continuitydb:sole-invalidated-trajectory", 0.99, 20)?;
        cell.set_trajectory_memory(TrajectoryMemory::new(
            "retry release upload before checking release existence",
            "old rollout trace found the missing release target",
            "upload returned 404 before the release was created",
            "artifact://trajectory/invalidated-sole",
            0.99,
            "verify the GitHub Release exists before retrying upload",
            vec!["retry release upload".to_string()],
            vec!["release existence has already been verified".to_string()],
            ContextPacketStrategy::FalsificationBrief,
        )?);
        append_committed(&mut kernel, cell)?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: Some(SemanticAnchor::new(
                    "project:continuitydb:sole-invalidated-trajectory",
                )),
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
                answerability_question: None,
                compiler_intent: Some(
                    "retry release upload after release existence has already been verified"
                        .to_string(),
                ),
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Reflection,
                compiler_policy: ContextCompilerPolicy::Automatic,
                minimum_confidence: Confidence::new(0.8)?,
                token_budget: 20,
            },
        )?;

        let packet = &slice.context_packets[0];
        assert!(!packet
            .entries
            .iter()
            .any(|entry| entry.source == ContextPacketEntrySource::NativeTrajectoryMemory));
        assert!(!packet
            .compiler_reason_tags
            .contains(&"trajectory-applicability-match".to_string()));
        assert_ne!(
            packet.plan.as_ref().map(|plan| plan.purpose),
            Some(ContextPacketPurpose::TrajectoryReuse)
        );
        Ok(())
    }

    #[test]
    fn checkout_packet_planner_materializes_lifecycle_hard_gate_when_cell_cost_is_tiny(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let mut cell = sample_cell("project:continuitydb:tiny-cost-lifecycle", 0.94, 4)?;
        cell.set_lifecycle_policy(ContextLifecyclePolicy {
            retention: RetentionPolicy::Persistent,
            use_policy: UsePolicy::DoNotUseForAnswer,
            promotion: PromotionPolicy::Manual,
        });
        append_committed(&mut kernel, cell)?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: Some(SemanticAnchor::new(
                    "project:continuitydb:tiny-cost-lifecycle",
                )),
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
                answerability_question: None,
                compiler_intent: Some("execute the release answer".to_string()),
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Execution,
                compiler_policy: ContextCompilerPolicy::Automatic,
                minimum_confidence: Confidence::new(0.8)?,
                token_budget: 12,
            },
        )?;

        let packet = &slice.context_packets[0];
        assert!(packet
            .lines
            .iter()
            .any(|line| line.contains("do not use for answer")));
        assert!(packet
            .entries
            .iter()
            .any(|entry| entry.source == ContextPacketEntrySource::NativeLifecyclePolicy));
        Ok(())
    }

    #[test]
    fn checkout_compiler_intent_does_not_change_selected_cells(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let mut applicable = sample_cell("project:continuitydb:intent-selection-low", 0.8, 8)?;
        applicable.set_trajectory_memory(TrajectoryMemory::new(
            "retry release upload from CI",
            "old trace found release upload retry behavior",
            "upload failed before release existence was verified",
            "artifact://trajectory/intent-low",
            0.99,
            "retry upload only after checking the release target",
            vec!["retry release upload".to_string()],
            vec!["release existence has already been verified".to_string()],
            ContextPacketStrategy::FalsificationBrief,
        )?);
        append_committed(&mut kernel, applicable)?;
        let high_confidence = append_committed(
            &mut kernel,
            sample_cell("project:continuitydb:intent-selection-high", 0.98, 8)?,
        )?;

        let no_intent = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: Some(Scope::Project("continuitydb".to_string())),
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
                answerability_question: None,
                compiler_intent: None,
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Reflection,
                compiler_policy: ContextCompilerPolicy::Automatic,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 8,
            },
        )?;
        let with_intent = checkout(
            &kernel,
            CheckoutRequest {
                compiler_intent: Some("retry release upload from CI".to_string()),
                ..CheckoutRequest {
                    semantic_anchor: None,
                    scope: Some(Scope::Project("continuitydb".to_string())),
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
                    answerability_question: None,
                    compiler_intent: None,
                    compiler_proposals: Vec::new(),
                    evidence_source: None,
                    dependency_target: None,
                    dependency_kind: None,
                    revision_related_cell: None,
                    revision_link_kind: None,
                    context_profile: ContextProfile::Reflection,
                    compiler_policy: ContextCompilerPolicy::Automatic,
                    minimum_confidence: Confidence::new(0.7)?,
                    token_budget: 8,
                }
            },
        )?;

        assert_eq!(no_intent.cells, vec![high_confidence.clone()]);
        assert_eq!(with_intent.cells, no_intent.cells);
        Ok(())
    }

    #[test]
    fn checkout_automatic_context_compiler_uses_native_scavenging_pressure(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let mut cell = sample_cell("project:continuitydb:scavenging-pressure", 0.86, 12)?;
        cell.set_uncertainty(EpistemicUncertainty::new(
            Confidence::new(0.82)?,
            0.5,
            "high uncertainty remains after partial evidence",
        )?);
        cell.set_attention(AttentionSignal::new(0.9, 0.8, 0.7, 0.8)?);
        cell.add_projection(MemoryProjection::new(
            MemoryProjectionKind::Semantic,
            "Current claim: migration finished except for unknown replica lag.",
            Confidence::new(0.86)?,
            CellCost::new(10, 0)?,
        )?);
        append_committed(&mut kernel, cell)?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: Some(SemanticAnchor::new(
                    "project:continuitydb:scavenging-pressure",
                )),
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
                answerability_question: None,
                compiler_intent: None,
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Planning,
                compiler_policy: ContextCompilerPolicy::Automatic,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 12,
            },
        )?;

        let packet = &slice.context_packets[0];
        assert_eq!(packet.compiler_policy, ContextCompilerPolicy::Automatic);
        assert_eq!(packet.strategy, ContextPacketStrategy::ScavengingBrief);
        assert_eq!(
            packet.abstraction_level,
            ContextAbstractionLevel::Scavenging
        );
        assert!(packet
            .compiler_reason_tags
            .contains(&"epistemic-scavenging-pressure".to_string()));
        assert!(packet
            .lines
            .first()
            .is_some_and(|line| line.starts_with("Uncertainty warning")));
        assert!(packet.token_count <= 12);
        Ok(())
    }

    #[test]
    fn checkout_automatic_context_compiler_uses_native_revision_pressure(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let mut cell = sample_cell("project:continuitydb:revision-pressure", 0.86, 12)?;
        cell.set_uncertainty(EpistemicUncertainty::new(
            Confidence::new(0.30)?,
            4.0,
            "low-probability rollout outcome occurred",
        )?);
        cell.add_projection(MemoryProjection::new(
            MemoryProjectionKind::Semantic,
            "Current claim: rollout path is still viable.",
            Confidence::new(0.86)?,
            CellCost::new(10, 0)?,
        )?);
        append_committed(&mut kernel, cell)?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: Some(SemanticAnchor::new(
                    "project:continuitydb:revision-pressure",
                )),
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
                answerability_question: None,
                compiler_intent: None,
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Planning,
                compiler_policy: ContextCompilerPolicy::Automatic,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 12,
            },
        )?;

        let packet = &slice.context_packets[0];
        assert_eq!(packet.compiler_policy, ContextCompilerPolicy::Automatic);
        assert_eq!(packet.strategy, ContextPacketStrategy::RevisionCapsule);
        assert_eq!(packet.abstraction_level, ContextAbstractionLevel::Capsule);
        assert!(packet
            .compiler_reason_tags
            .contains(&"epistemic-revision-pressure".to_string()));
        assert!(packet
            .lines
            .first()
            .is_some_and(|line| line.starts_with("Uncertainty warning")));
        assert!(packet.token_count <= 12);
        Ok(())
    }

    #[test]
    fn checkout_automatic_context_compiler_prioritizes_highest_context_gap(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let mut cell = sample_cell("project:continuitydb:gap-priority", 0.89, 4)?;
        cell.add_context_gap(ContextGap::new(
            ContextGapKind::MissingEvidence,
            "which dashboard screenshot is archived?",
            "useful but not deployment blocking",
            0.3,
        )?);
        cell.add_context_gap(ContextGap::new(
            ContextGapKind::MissingDependency,
            "which database migration dependency is still unverified?",
            "must be answered before claiming rollout readiness",
            0.95,
        )?);
        append_committed(&mut kernel, cell)?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: Some(SemanticAnchor::new("project:continuitydb:gap-priority")),
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
                answerability_question: None,
                compiler_intent: Some("is the rollout ready?".to_string()),
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Planning,
                compiler_policy: ContextCompilerPolicy::Automatic,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 8,
            },
        )?;

        let packet = &slice.context_packets[0];
        assert_eq!(packet.strategy, ContextPacketStrategy::ScavengingBrief);
        assert_eq!(
            packet.lines.first().map(String::as_str),
            Some("Missing context: which database migration dependency is still unverified?")
        );
        assert!(packet.token_count <= 8);
        Ok(())
    }

    #[test]
    fn checkout_automatic_context_compiler_prioritizes_highest_invalidation_condition(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let mut cell = sample_cell("project:continuitydb:invalidation-priority", 0.89, 4)?;
        cell.add_invalidation_condition(InvalidationCondition::new(
            InvalidationConditionKind::TemporalExpiry,
            "the observation is older than one week",
            "stale evidence matters but is not the primary blocker",
            0.4,
        )?);
        cell.add_invalidation_condition(InvalidationCondition::new(
            InvalidationConditionKind::BoundaryViolation,
            "the migration ran outside the approved release window",
            "this invalidates using the claim for deployment action",
            0.95,
        )?);
        append_committed(&mut kernel, cell)?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: Some(SemanticAnchor::new(
                    "project:continuitydb:invalidation-priority",
                )),
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
                answerability_question: None,
                compiler_intent: Some("what should I do next for this deployment?".to_string()),
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Planning,
                compiler_policy: ContextCompilerPolicy::Automatic,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 8,
            },
        )?;

        let packet = &slice.context_packets[0];
        assert_eq!(packet.strategy, ContextPacketStrategy::FalsificationBrief);
        assert_eq!(
            packet.lines.first().map(String::as_str),
            Some("Invalidation condition: the migration ran outside the approved release window")
        );
        assert!(packet.token_count <= 8);
        Ok(())
    }

    #[test]
    fn checkout_automatic_context_compiler_records_native_decision_evidence(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let mut cell = sample_cell("project:continuitydb:adaptive-native-evidence", 0.88, 12)?;
        cell.set_context_affordance(ContextAffordance::new(0.80, 0.85, 0.95, 0.90, 0.75, 0.10)?);
        append_committed(&mut kernel, cell)?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: Some(SemanticAnchor::new(
                    "project:continuitydb:adaptive-native-evidence",
                )),
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
                answerability_question: None,
                compiler_intent: None,
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Planning,
                compiler_policy: ContextCompilerPolicy::Automatic,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 12,
            },
        )?;

        let packet = &slice.context_packets[0];
        assert!(packet
            .compiler_reason_tags
            .contains(&"context-affordance-risk".to_string()));
        assert_eq!(
            packet.compiler_evidence_locators,
            vec!["test://project:continuitydb:adaptive-native-evidence".to_string()]
        );
        assert!(packet
            .compiler_evidence_locators
            .iter()
            .all(|locator| packet.citations.contains(locator)));
        Ok(())
    }

    #[test]
    fn checkout_automatic_revision_context_carries_related_evidence(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let selected = append_committed(
            &mut kernel,
            sample_cell("project:continuitydb:revision-evidence-selected", 0.91, 12)?,
        )?;
        let related = append_committed(
            &mut kernel,
            sample_cell("project:continuitydb:revision-evidence-related", 0.82, 12)?,
        )?;
        kernel.append_revision_link(RevisionLinkRecord::new(
            selected.id,
            RevisionLinkKind::Supersedes,
            related.id,
            test_commit_time()?,
        ))?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: Some(SemanticAnchor::new(
                    "project:continuitydb:revision-evidence-selected",
                )),
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
                answerability_question: None,
                compiler_intent: Some("what changed?".to_string()),
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Planning,
                compiler_policy: ContextCompilerPolicy::Automatic,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 12,
            },
        )?;

        let packet = &slice.context_packets[0];
        let selected_locator = "test://project:continuitydb:revision-evidence-selected".to_string();
        let related_locator = "test://project:continuitydb:revision-evidence-related".to_string();

        assert!(packet
            .compiler_reason_tags
            .contains(&"revision-context-present".to_string()));
        assert!(packet
            .compiler_evidence_locators
            .contains(&selected_locator));
        assert!(packet.compiler_evidence_locators.contains(&related_locator));
        assert!(packet.citations.contains(&selected_locator));
        assert!(packet.citations.contains(&related_locator));
        assert_eq!(
            packet.revision_context[0].anchors,
            vec![SemanticAnchor::new(
                "project:continuitydb:revision-evidence-related"
            )]
        );
        assert!(packet
            .lines
            .iter()
            .any(|line| line.contains("project:continuitydb:revision-evidence-related")));
        Ok(())
    }

    #[test]
    fn checkout_automatic_dependency_context_carries_target_evidence(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let dependency = append_committed(
            &mut kernel,
            sample_cell("project:continuitydb:adaptive-dependency-target", 0.82, 10)?,
        )?;
        let mut dependent = sample_cell(
            "project:continuitydb:adaptive-dependency-selected",
            0.91,
            12,
        )?;
        dependent.dependencies.push(CellDependency::new(
            dependency.id,
            CellDependencyKind::DependsOn,
            "rollout readiness depends on migration completion",
        ));
        append_committed(&mut kernel, dependent)?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: Some(SemanticAnchor::new(
                    "project:continuitydb:adaptive-dependency-selected",
                )),
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
                answerability_question: None,
                compiler_intent: Some("is the rollout ready given dependencies?".to_string()),
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Planning,
                compiler_policy: ContextCompilerPolicy::Automatic,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 12,
            },
        )?;

        let packet = &slice.context_packets[0];
        let selected_locator =
            "test://project:continuitydb:adaptive-dependency-selected".to_string();
        let dependency_locator =
            "test://project:continuitydb:adaptive-dependency-target".to_string();

        assert_eq!(
            packet.abstraction_level,
            ContextAbstractionLevel::EvidenceDense
        );
        assert!(packet
            .compiler_reason_tags
            .contains(&"dependency-context-present".to_string()));
        assert!(packet
            .compiler_evidence_locators
            .contains(&selected_locator));
        assert!(packet
            .compiler_evidence_locators
            .contains(&dependency_locator));
        assert!(packet.citations.contains(&dependency_locator));
        assert!(packet.lines.iter().any(|line| line.contains(
            "Dependency context: DependsOn rollout readiness depends on migration completion"
        )));
        assert!(packet
            .lines
            .iter()
            .any(|line| line.contains("project:continuitydb:adaptive-dependency-target")));
        assert!(packet.entries.iter().any(|entry| {
            entry.source == ContextPacketEntrySource::NativeDependencyContext
                && entry
                    .text
                    .contains("project:continuitydb:adaptive-dependency-target")
        }));
        Ok(())
    }

    #[test]
    fn checkout_slice_materializes_requested_debugging_context_packets(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let mut cell = sample_cell("project:continuitydb:debugging-profile", 0.95, 17)?;
        cell.add_projection(MemoryProjection::new(
            MemoryProjectionKind::Episodic,
            "Observed HTTP 404 while uploading release asset.",
            Confidence::new(0.99)?,
            CellCost::new(9, 0)?,
        )?);
        cell.add_projection(MemoryProjection::new(
            MemoryProjectionKind::Semantic,
            "Current belief: release lookup returned a stale upload URL.",
            Confidence::new(0.91)?,
            CellCost::new(8, 0)?,
        )?);
        cell.add_projection(MemoryProjection::new(
            MemoryProjectionKind::Uncertainty,
            "Open question: release id may differ from the asset upload target.",
            Confidence::new(0.70)?,
            CellCost::new(8, 0)?,
        )?);
        let cell = append_committed(&mut kernel, cell)?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: Some(Scope::Project("continuitydb".to_string())),
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
                answerability_question: None,
                compiler_intent: None,
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Debugging,
                compiler_policy: ContextCompilerPolicy::RawBaseline,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 17,
            },
        )?;

        assert_eq!(slice.cells, vec![cell]);
        assert_eq!(slice.context_packets[0].profile, ContextProfile::Debugging);
        assert_eq!(
            slice.context_packets[0].lines,
            vec![
                "Observed HTTP 404 while uploading release asset.".to_string(),
                "Open question: release id may differ from the asset upload target.".to_string(),
            ]
        );
        Ok(())
    }

    #[test]
    fn checkout_slice_summarizes_bounded_evidence_context() -> Result<(), Box<dyn std::error::Error>>
    {
        let mut kernel = MemoryKernel::default();
        let selected = append_committed(
            &mut kernel,
            sample_cell("project:continuitydb:summary-selected", 0.95, 10)?,
        )?;
        let mut frontier = sample_cell("project:continuitydb:summary-frontier", 0.72, 10)?;
        frontier.activation = ActivationState::Frontier;
        let expectation = EpistemicExpectation::from_expected_outcome(
            "release asset exists",
            Confidence::new(0.9)?,
            false,
        )?;
        frontier.set_uncertainty(EpistemicUncertainty::from_expectation(
            Confidence::new(0.82)?,
            expectation,
            "strong baseline failed and needs missing evidence",
        )?);
        let frontier = append_committed(&mut kernel, frontier)?;
        let omitted = append_committed(
            &mut kernel,
            sample_cell("project:continuitydb:summary-omitted", 0.90, 15)?,
        )?;
        let related = append_committed(
            &mut kernel,
            sample_cell("project:continuitydb:summary-related", 0.69, 10)?,
        )?;
        kernel.append_revision_link(RevisionLinkRecord::new(
            selected.id,
            RevisionLinkKind::ConflictsWith,
            related.id,
            test_commit_time()?,
        ))?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: Some(Scope::Project("continuitydb".to_string())),
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
                answerability_question: None,
                compiler_intent: None,
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Execution,
                compiler_policy: ContextCompilerPolicy::RawBaseline,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 20,
            },
        )?;

        assert_eq!(slice.cells, vec![selected, frontier]);
        assert_eq!(slice.alternatives[0].cell_id, omitted.id);
        assert_eq!(slice.summary.selected_cell_count, 2);
        assert_eq!(slice.summary.alternative_count, 1);
        assert_eq!(slice.summary.total_tokens, 20);
        assert_eq!(slice.summary.token_budget, 20);
        assert_eq!(slice.summary.citation_count, 2);
        assert_eq!(slice.summary.uncertainty_count, 2);
        assert_eq!(slice.summary.frontier_recommendation_count, 1);
        assert_eq!(slice.summary.revision_link_count, 1);
        assert_eq!(slice.summary.revision_context_count, 1);
        assert_eq!(
            slice
                .summary
                .epistemic_action_counts
                .iter()
                .map(|summary| (summary.action, summary.count))
                .collect::<Vec<_>>(),
            vec![(EpistemicAction::Use, 1), (EpistemicAction::Scavenge, 1)]
        );
        assert_eq!(
            slice
                .summary
                .epistemic_action_reason_counts
                .iter()
                .map(|summary| (summary.reason, summary.count))
                .collect::<Vec<_>>(),
            vec![
                (EpistemicActionReason::HighUncertainty, 1),
                (EpistemicActionReason::HighSurprise, 1)
            ]
        );
        assert_eq!(
            slice
                .summary
                .selection_reason_counts
                .iter()
                .map(|summary| (summary.reason, summary.count))
                .collect::<Vec<_>>(),
            vec![
                (ContextPacketSelectionReason::EvidenceConfidence, 2),
                (ContextPacketSelectionReason::Answerability, 2)
            ]
        );
        assert!(
            (slice.summary.epistemic_pressure.maximum_revision_pressure - 0.597_947).abs()
                < 0.000_01
        );
        assert!(
            (slice.summary.epistemic_pressure.maximum_scavenging_pressure - 0.41).abs() < 0.000_01
        );
        assert!(
            (slice.summary.epistemic_pressure.maximum_checkout_pressure - 0.597_947).abs()
                < 0.000_01
        );
        assert_eq!(
            slice.summary.minimum_selected_confidence,
            Some(Confidence::new(0.72)?)
        );
        assert_eq!(
            slice.summary.maximum_selected_confidence,
            Some(Confidence::new(0.95)?)
        );
        assert!(slice.summary.bounded_by_token_budget);
        Ok(())
    }

    #[test]
    fn checkout_audit_traces_include_native_revision_links(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let selected = append_committed(
            &mut kernel,
            sample_cell("project:continuitydb:checkout-link-selected", 0.95, 10)?,
        )?;
        let superseded = append_committed(
            &mut kernel,
            sample_cell("project:continuitydb:checkout-link-superseded", 0.83, 10)?,
        )?;
        let predecessor = append_committed(
            &mut kernel,
            sample_cell("project:continuitydb:checkout-link-predecessor", 0.72, 10)?,
        )?;
        let committed_at = test_commit_time()?;
        let source_link = RevisionLinkRecord::new(
            selected.id,
            RevisionLinkKind::Supersedes,
            superseded.id,
            committed_at,
        );
        let target_link = RevisionLinkRecord::new(
            predecessor.id,
            RevisionLinkKind::Predecessor,
            selected.id,
            committed_at,
        );
        kernel.append_revision_link(source_link.clone())?;
        kernel.append_revision_link(target_link.clone())?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: Some(SemanticAnchor::new(
                    "project:continuitydb:checkout-link-selected",
                )),
                scope: Some(Scope::Project("continuitydb".to_string())),
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
                answerability_question: None,
                compiler_intent: None,
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Execution,
                compiler_policy: ContextCompilerPolicy::RawBaseline,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 20,
            },
        )?;

        assert_eq!(slice.cells, vec![selected]);
        assert_eq!(
            slice.audit_traces[0].revision_links,
            vec![source_link, target_link]
        );
        let superseded_confidence = Confidence::new(0.83)?;
        let predecessor_confidence = Confidence::new(0.72)?;
        assert_eq!(slice.audit_traces[0].revision_context.len(), 2);
        assert!(slice.audit_traces[0]
            .revision_context
            .iter()
            .any(|context| {
                context.related_cell_id == superseded.id
                    && context.relation == super::AuditRevisionRelation::SourceToTarget
                    && context.kind == RevisionLinkKind::Supersedes
                    && context.citations
                        == vec!["test://project:continuitydb:checkout-link-superseded".to_string()]
                    && context.max_confidence == superseded_confidence
            }));
        assert!(slice.audit_traces[0]
            .revision_context
            .iter()
            .any(|context| {
                context.related_cell_id == predecessor.id
                    && context.relation == super::AuditRevisionRelation::TargetFromSource
                    && context.kind == RevisionLinkKind::Predecessor
                    && context.citations
                        == vec!["test://project:continuitydb:checkout-link-predecessor".to_string()]
                    && context.max_confidence == predecessor_confidence
            }));
        assert_eq!(slice.context_packets[0].revision_context.len(), 2);
        assert!(slice.context_packets[0]
            .revision_context
            .iter()
            .any(|context| {
                context.related_cell_id == superseded.id
                    && context.kind == RevisionLinkKind::Supersedes
                    && context.citations
                        == vec!["test://project:continuitydb:checkout-link-superseded".to_string()]
                    && context.max_confidence == superseded_confidence
            }));
        Ok(())
    }

    #[test]
    fn checkout_audit_traces_deduplicate_self_revision_links(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let selected = append_committed(
            &mut kernel,
            sample_cell("project:continuitydb:checkout-link-self", 0.95, 10)?,
        )?;
        let committed_at = test_commit_time()?;
        let self_link = RevisionLinkRecord::new(
            selected.id,
            RevisionLinkKind::DerivesFrom,
            selected.id,
            committed_at,
        );
        kernel.append_revision_link(self_link.clone())?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: Some(SemanticAnchor::new(
                    "project:continuitydb:checkout-link-self",
                )),
                scope: Some(Scope::Project("continuitydb".to_string())),
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
                answerability_question: None,
                compiler_intent: None,
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Execution,
                compiler_policy: ContextCompilerPolicy::RawBaseline,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 20,
            },
        )?;

        assert_eq!(slice.cells, vec![selected]);
        assert_eq!(slice.audit_traces[0].revision_links, vec![self_link]);
        assert_eq!(slice.audit_traces[0].revision_context.len(), 1);
        assert_eq!(
            slice.audit_traces[0].revision_context[0].relation,
            super::AuditRevisionRelation::SelfLink
        );
        Ok(())
    }

    #[test]
    fn checkout_filters_by_revision_related_cell_and_kind() -> Result<(), Box<dyn std::error::Error>>
    {
        let mut kernel = MemoryKernel::default();
        let selected = append_committed(
            &mut kernel,
            sample_cell("project:continuitydb:revision-filter-selected", 0.95, 10)?,
        )?;
        let related = append_committed(
            &mut kernel,
            sample_cell("project:continuitydb:revision-filter-related", 0.80, 10)?,
        )?;
        let unrelated = append_committed(
            &mut kernel,
            sample_cell("project:continuitydb:revision-filter-unrelated", 0.90, 10)?,
        )?;
        let committed_at = test_commit_time()?;
        kernel.append_revision_link(RevisionLinkRecord::new(
            selected.id,
            RevisionLinkKind::ConflictsWith,
            related.id,
            committed_at,
        ))?;
        kernel.append_revision_link(RevisionLinkRecord::new(
            unrelated.id,
            RevisionLinkKind::Supersedes,
            related.id,
            committed_at,
        ))?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: Some(Scope::Project("continuitydb".to_string())),
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
                answerability_question: None,
                compiler_intent: None,
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: Some(related.id),
                revision_link_kind: Some(RevisionLinkKind::ConflictsWith),
                context_profile: ContextProfile::Execution,
                compiler_policy: ContextCompilerPolicy::RawBaseline,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 100,
            },
        )?;

        assert_eq!(slice.cells, vec![selected]);
        assert_eq!(
            slice.audit_traces[0].revision_context[0].related_cell_id,
            related.id
        );
        Ok(())
    }

    #[test]
    fn checkout_records_token_budget_alternatives() -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let selected = sample_cell("project:continuitydb:selected", 0.95, 10)?;
        let omitted = sample_cell("project:continuitydb:omitted", 0.90, 10)?;
        let selected = append_committed(&mut kernel, selected)?;
        let omitted = append_committed(&mut kernel, omitted)?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: Some(Scope::Project("continuitydb".to_string())),
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
                answerability_question: None,
                compiler_intent: None,
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Execution,
                compiler_policy: ContextCompilerPolicy::RawBaseline,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 10,
            },
        )?;

        assert_eq!(slice.cells, vec![selected]);
        assert_eq!(slice.alternatives.len(), 1);
        assert_eq!(slice.alternatives[0].cell_id, omitted.id);
        assert_eq!(
            slice.alternatives[0].reason,
            super::CheckoutAlternativeReason::TokenBudgetExceeded
        );
        assert_eq!(
            slice.alternatives[0].citations,
            vec!["test://project:continuitydb:omitted".to_string()]
        );
        assert_eq!(slice.alternatives[0].max_confidence, Confidence::new(0.90)?);
        Ok(())
    }

    #[test]
    fn audit_includes_cell_id_and_citation_locator() -> Result<(), Box<dyn std::error::Error>> {
        let cell = sample_cell("project:continuitydb:audit", 0.95, 10)?;
        let trace = audit(&cell);

        assert_eq!(trace.cell_id, cell.id);
        assert_eq!(
            trace.citations,
            vec!["test://project:continuitydb:audit".to_string()]
        );
        assert!(trace.dependencies.is_empty());
        Ok(())
    }

    #[test]
    fn audit_includes_commit_id() -> Result<(), Box<dyn std::error::Error>> {
        let mut cell = sample_cell("project:continuitydb:audit-commit", 0.95, 10)?;
        cell.commit_id = CommitId::new();

        let trace = audit(&cell);

        assert_eq!(trace.commit_id, cell.commit_id);
        Ok(())
    }

    #[test]
    fn audit_includes_structured_evidence() -> Result<(), Box<dyn std::error::Error>> {
        let cell = sample_cell("project:continuitydb:audit-evidence", 0.95, 10)?;
        let trace = audit(&cell);

        assert_eq!(trace.evidence.len(), 1);
        assert_eq!(trace.evidence[0].source, "test");
        assert_eq!(
            trace.evidence[0].locator,
            "test://project:continuitydb:audit-evidence"
        );
        assert_eq!(trace.evidence[0].confidence, Confidence::new(0.95)?);
        assert_eq!(
            trace.evidence[0].trust,
            vec![TrustSignal::DirectObservation]
        );
        Ok(())
    }

    #[test]
    fn audit_includes_dependency_links() -> Result<(), Box<dyn std::error::Error>> {
        let target = StateCellId::new();
        let mut cell = sample_cell("project:continuitydb:audit-dependency", 0.95, 10)?;
        cell.dependencies.push(CellDependency::new(
            target,
            CellDependencyKind::DerivedFrom,
            "derived from source evidence",
        ));

        let trace = audit(&cell);

        assert_eq!(trace.dependencies.len(), 1);
        assert_eq!(trace.dependencies[0].target, target);
        assert_eq!(trace.dependencies[0].kind, CellDependencyKind::DerivedFrom);
        assert_eq!(
            trace.dependencies[0].rationale,
            "derived from source evidence"
        );
        Ok(())
    }

    #[test]
    fn checkout_audit_traces_include_dependency_links() -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let target = StateCellId::new();
        let mut dependent = sample_cell("project:continuitydb:audit-dependent", 0.95, 10)?;
        dependent.dependencies.push(CellDependency::new(
            target,
            CellDependencyKind::DependsOn,
            "depends on target state",
        ));
        append_committed(&mut kernel, dependent)?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: Some(Scope::Project("continuitydb".to_string())),
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
                answerability_question: None,
                compiler_intent: None,
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Execution,
                compiler_policy: ContextCompilerPolicy::RawBaseline,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 10,
            },
        )?;

        assert_eq!(slice.audit_traces.len(), 1);
        assert_eq!(slice.audit_traces[0].dependencies.len(), 1);
        assert_eq!(slice.audit_traces[0].dependencies[0].target, target);
        assert_eq!(
            slice.audit_traces[0].dependencies[0].kind,
            CellDependencyKind::DependsOn
        );
        assert_eq!(
            slice.audit_traces[0].dependencies[0].rationale,
            "depends on target state"
        );
        Ok(())
    }

    #[test]
    fn checkout_context_packets_preserve_dependency_context(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let target_cell = append_committed(
            &mut kernel,
            sample_cell("project:continuitydb:packet-dependency-target", 0.82, 10)?,
        )?;
        let mut dependent = sample_cell("project:continuitydb:packet-dependent", 0.95, 10)?;
        dependent.dependencies.push(CellDependency::new(
            target_cell.id,
            CellDependencyKind::DependsOn,
            "depends on target state",
        ));
        append_committed(&mut kernel, dependent)?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: Some(Scope::Project("continuitydb".to_string())),
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
                answerability_question: None,
                compiler_intent: None,
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Execution,
                compiler_policy: ContextCompilerPolicy::RawBaseline,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 10,
            },
        )?;

        assert_eq!(slice.context_packets.len(), 1);
        assert_eq!(slice.context_packets[0].dependency_context.len(), 1);
        assert_eq!(
            slice.context_packets[0].dependency_context[0].target,
            target_cell.id
        );
        assert_eq!(
            slice.context_packets[0].dependency_context[0].kind,
            CellDependencyKind::DependsOn
        );
        assert_eq!(
            slice.context_packets[0].dependency_context[0].rationale,
            "depends on target state"
        );
        assert_eq!(
            slice.context_packets[0].dependency_context[0].citations,
            vec!["test://project:continuitydb:packet-dependency-target".to_string()]
        );
        assert_eq!(
            slice.context_packets[0].dependency_context[0].anchors,
            vec![SemanticAnchor::new(
                "project:continuitydb:packet-dependency-target"
            )]
        );
        assert!(slice.context_packets[0]
            .citations
            .contains(&"test://project:continuitydb:packet-dependency-target".to_string()));
        Ok(())
    }

    #[test]
    fn checkout_audit_traces_include_commit_ids() -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let selected = append_committed(
            &mut kernel,
            sample_cell("project:continuitydb:audit-commit-checkout", 0.95, 10)?,
        )?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                semantic_anchor: None,
                scope: Some(Scope::Project("continuitydb".to_string())),
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
                answerability_question: None,
                compiler_intent: None,
                compiler_proposals: Vec::new(),
                evidence_source: None,
                dependency_target: None,
                dependency_kind: None,
                revision_related_cell: None,
                revision_link_kind: None,
                context_profile: ContextProfile::Execution,
                compiler_policy: ContextCompilerPolicy::RawBaseline,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 10,
            },
        )?;

        assert_eq!(slice.audit_traces.len(), 1);
        assert_eq!(slice.audit_traces[0].cell_id, selected.id);
        assert_eq!(slice.audit_traces[0].commit_id, selected.commit_id);
        Ok(())
    }
}
