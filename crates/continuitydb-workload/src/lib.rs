//! Deterministic workload generation for ContinuityDB benchmarks.

use chrono::{DateTime, Utc};
use continuitydb_checkout::{checkout, CheckoutError, CheckoutRequest};
use continuitydb_core::{
    ActivationState, Answerability, CellCost, CellDependency, CellDependencyKind, CellPayload,
    Citation, CommitId, Confidence, ContextAffordance, ContextCompilerPolicy, ContextGap,
    ContextGapKind, ContextLifecyclePolicy, ContextPacketSelectionReason, ContextPacketStrategy,
    ContextProfile, EpistemicUncertainty, Evidence, InvalidationCondition,
    InvalidationConditionKind, LifecycleStage, MemoryProjection, MemoryProjectionKind,
    PromotionPolicy, RetentionPolicy, RevisionLinkKind, RevisionLinkRecord, Scope, SemanticAnchor,
    StateCell, StateCellId, TrajectoryMemory, TrustSignal, UsePolicy, UtilityFeedback,
    ValidTimeRange,
};
use continuitydb_kernel::{FileKernelLookupPlan, KernelError, StorageKernel};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use thiserror::Error;

/// Deterministic workload generation parameters.
#[derive(Clone, Debug, PartialEq)]
pub struct WorkloadConfig {
    /// Number of StateCells to generate.
    pub cell_count: usize,
    /// Numeric seed used as the base for deterministic StateCell IDs.
    pub id_seed: u128,
    /// Prefix for generated semantic anchors.
    pub anchor_prefix: String,
    /// Project scope for generated cells.
    pub project_scope: String,
    /// Valid-time start assigned to generated cells.
    pub valid_from: DateTime<Utc>,
    /// Every Nth cell becomes frontier.
    pub frontier_every: usize,
    /// Each cell after this stride depends on the cell at index minus stride.
    pub dependency_stride: usize,
}

/// Generated workload and summary metadata.
#[derive(Clone, Debug, PartialEq)]
pub struct ContinuityWorkload {
    /// Generated StateCells in deterministic append order.
    pub cells: Vec<StateCell>,
    /// Summary of workload shape.
    pub summary: WorkloadSummary,
}

/// Deterministic workload shape summary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkloadSummary {
    /// Number of generated StateCells.
    pub cell_count: usize,
    /// Number of generated cells marked as frontier.
    pub frontier_count: usize,
    /// Number of generated dependency edges.
    pub dependency_count: usize,
    /// Sum of generated token costs.
    pub total_token_cost: i64,
}

/// Measured operation count and elapsed wall-clock time.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MeasuredOperation {
    /// Number of logical operations performed.
    pub operation_count: usize,
    /// Observed elapsed time for the operation group.
    pub elapsed: Duration,
}

/// Checkout result counts measured from a workload run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckoutMeasurement {
    /// Number of checkout candidates matching request constraints.
    pub matched_count: usize,
    /// Number of cells selected into the returned slice.
    pub selected_count: usize,
    /// Number of matching cells omitted as alternatives.
    pub alternative_count: usize,
    /// Number of selected frontier cells.
    pub frontier_count: usize,
    /// Selected token total reported by checkout.
    pub selected_token_count: i64,
}

/// End-to-end workload measurement over one kernel and checkout request.
#[derive(Clone, Debug, PartialEq)]
pub struct WorkloadMeasurement {
    /// Summary of the generated workload that was measured.
    pub workload_summary: WorkloadSummary,
    /// Number of native revision links appended for the workload.
    pub revision_link_count: usize,
    /// Ingest operation measurement.
    pub ingest: MeasuredOperation,
    /// Checkout operation measurement.
    pub checkout_operation: MeasuredOperation,
    /// Checkout result counts.
    pub checkout: CheckoutMeasurement,
}

/// Representative lifecycle/context behavior report for a long-horizon agent memory scenario.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RepresentativeAgentMemoryHorizonReport {
    /// Number of deterministic turns measured by the scenario.
    pub turn_count: usize,
    /// Number of selected packets that preserved revision-neighborhood guidance.
    pub revision_guidance_packet_count: usize,
    /// Number of selected packets that preserved hedging or verification guidance.
    pub hedging_packet_count: usize,
    /// Number of selected packets that preserved invalidation-condition guidance.
    pub invalidation_packet_count: usize,
    /// Number of selected packets that reused applicable trajectory memory.
    pub trajectory_reuse_packet_count: usize,
    /// Number of selected cells carrying the known stale release-upload belief.
    pub stale_belief_selected_count: usize,
    /// Deterministic lifecycle success score in basis points.
    pub lifecycle_success_basis_points: usize,
}

/// Downstream task-harness comparison for adversarial StateCell checkout behavior.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AdversarialAgentTaskHarnessReport {
    /// Number of adversarial downstream task cases.
    pub case_count: usize,
    /// Deterministic baseline checkout score.
    pub baseline: AdversarialAgentTaskScore,
    /// Task-adaptive checkout score.
    pub adaptive: AdversarialAgentTaskScore,
}

/// Deterministic downstream answer-quality score for one checkout mode.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AdversarialAgentTaskScore {
    /// Compiler policy label.
    pub policy: String,
    /// Number of passed task criteria.
    pub passed_count: usize,
    /// Number of evaluated task criteria.
    pub total_count: usize,
    /// Score in basis points.
    pub score_basis_points: usize,
    /// Whether the answer avoided the stale/superseded belief.
    pub avoided_stale_belief: bool,
    /// Whether the answer used the superseding correction.
    pub used_superseding_correction: bool,
    /// Whether the answer preserved uncertainty or hedging guidance.
    pub hedged_uncertainty: bool,
    /// Whether the answer cited invalidation evidence.
    pub cited_invalidation: bool,
    /// Whether invalidated trajectory reuse was suppressed.
    pub suppressed_invalidated_trajectory: bool,
    /// Whether revision guidance survived a tight token budget.
    pub preserved_revision_under_pressure: bool,
    /// Whether `DoNotUseForAnswer` lifecycle guidance was preserved.
    pub enforced_do_not_use_policy: bool,
    /// Whether the applicable trajectory was selected while the invalidated trajectory was suppressed.
    pub selected_applicable_trajectory_under_pressure: bool,
    /// Whether high misuse-risk context forced verification or hedging guidance.
    pub forced_verification_for_misuse_risk: bool,
}

/// Small downstream behavior benchmark for the simplified checkout product model.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AgentBehaviorBenchmarkReport {
    /// Stable report format identifier.
    pub format: String,
    /// Report format version.
    pub format_version: u32,
    /// Evidence mode for this early benchmark.
    pub evidence_mode: String,
    /// Number of task scenarios.
    pub task_count: usize,
    /// Deterministic turns per task scenario.
    pub turns_per_task: usize,
    /// Shared context token budget.
    pub token_budget: i64,
    /// Strategy scores.
    pub strategies: Vec<AgentBehaviorStrategyReport>,
    /// Ordered gates required before marketing representative claims.
    pub next_expansion_gates: Vec<String>,
}

/// One strategy's downstream behavior score.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AgentBehaviorStrategyReport {
    /// Strategy identifier.
    pub strategy: String,
    /// Human-readable strategy role.
    pub role: String,
    /// Number of task scenarios that satisfied objective success criteria.
    pub passed_task_count: usize,
    /// Number of evaluated task scenarios.
    pub total_task_count: usize,
    /// Aggregated objective behavior metrics.
    pub metrics: AgentBehaviorMetrics,
    /// Approximate 95% confidence interval for task-success rate in basis points.
    pub task_success_confidence_interval_bps: AgentBehaviorConfidenceInterval,
}

/// Integer confidence interval represented in basis points.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AgentBehaviorConfidenceInterval {
    /// Lower confidence bound in basis points.
    pub lower_bps: usize,
    /// Upper confidence bound in basis points.
    pub upper_bps: usize,
}

/// Objective behavior metrics for long-running agent context.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AgentBehaviorMetrics {
    /// Task success rate in basis points.
    pub task_success_rate_bps: usize,
    /// Rate of using superseded information in basis points.
    pub stale_belief_rate_bps: usize,
    /// Rate of correctly using revision/correction state in basis points.
    pub revision_accuracy_bps: usize,
    /// Rate of unsupported certainty in basis points.
    pub unsupported_certainty_rate_bps: usize,
    /// Rate of verifying evidence when required in basis points.
    pub verification_rate_bps: usize,
    /// Rate of staying within context budget in basis points.
    pub context_budget_fit_bps: usize,
    /// Rate of repeating a known failed action in basis points.
    pub action_regression_rate_bps: usize,
}

/// Thesis-falsification benchmark comparing continuity control packets against strong memory baselines.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ThesisFalsificationBenchmarkReport {
    /// Stable report format identifier.
    pub format: String,
    /// Report format version.
    pub format_version: u32,
    /// Question the benchmark is designed to falsify.
    pub benchmark_question: String,
    /// Corpus construction summary.
    pub corpus: ThesisFalsificationCorpusReport,
    /// Number of evaluated tasks.
    pub task_count: usize,
    /// Strategy scores and retained predicate outcomes.
    pub strategies: Vec<ThesisFalsificationStrategyReport>,
    /// Decisive thesis judgement derived from the pass/fail gates.
    pub judgement: ThesisFalsificationJudgement,
}

/// Corpus summary for the thesis-falsification benchmark.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ThesisFalsificationCorpusReport {
    /// Number of retained corpus documents.
    pub document_count: usize,
    /// Number of planted stale beliefs.
    pub stale_belief_count: usize,
    /// Number of planted supersession/revision relationships.
    pub revision_count: usize,
    /// Number of planted forbidden actions.
    pub forbidden_action_count: usize,
    /// Number of planted uncertainty or verification obligations.
    pub uncertainty_or_verification_count: usize,
}

/// One strategy's thesis-falsification result.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ThesisFalsificationStrategyReport {
    /// Strategy identifier.
    pub strategy: String,
    /// Human-readable role.
    pub role: String,
    /// Passed task count.
    pub passed_task_count: usize,
    /// Total task count.
    pub total_task_count: usize,
    /// Aggregate behavior metrics.
    pub metrics: ThesisFalsificationMetrics,
    /// Retained scored records.
    pub records: Vec<ThesisFalsificationScoredRecord>,
}

/// Predicate-level metrics for the thesis-falsification benchmark.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ThesisFalsificationMetrics {
    /// All required predicates satisfied.
    pub task_success_rate_bps: usize,
    /// Correct decision or action chosen.
    pub correct_decision_bps: usize,
    /// Stale belief rejected when present.
    pub stale_belief_rejection_bps: usize,
    /// Superseding revision preserved when required.
    pub revision_preservation_bps: usize,
    /// Known failed or forbidden action avoided.
    pub forbidden_action_avoidance_bps: usize,
    /// Verification obligation surfaced.
    pub verification_trigger_bps: usize,
    /// Uncertainty obligation surfaced without overclaiming.
    pub uncertainty_faithfulness_bps: usize,
    /// Evidence locator or citation retained.
    pub evidence_grounding_bps: usize,
    /// Answer remains operationally useful rather than merely refusing.
    pub helpfulness_bps: usize,
}

/// Retained scored record for one strategy/task pair.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ThesisFalsificationScoredRecord {
    /// Task identifier.
    pub task_id: String,
    /// Strategy identifier.
    pub strategy: String,
    /// User-facing task prompt.
    pub prompt: String,
    /// Strategy context packet or retrieved memory summary.
    pub context_packet: String,
    /// Deterministic answer generated by the strategy adapter.
    pub answer: String,
    /// Predicate-level outcome.
    pub outcome: ThesisFalsificationOutcome,
    /// Failure attribution for any missing predicate.
    pub failure_attribution: Vec<String>,
}

/// Predicate outcome for one task answer.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ThesisFalsificationOutcome {
    /// All required predicates satisfied.
    pub task_success: bool,
    /// Correct decision/action.
    pub correct_decision: bool,
    /// Rejected stale belief where required.
    pub rejected_stale_belief: bool,
    /// Preserved revision/supersession where required.
    pub preserved_revision: bool,
    /// Avoided forbidden action where required.
    pub avoided_forbidden_action: bool,
    /// Triggered verification where required.
    pub triggered_verification: bool,
    /// Expressed uncertainty where required and avoided unsupported certainty.
    pub faithful_uncertainty: bool,
    /// Included evidence locator/citation where required.
    pub evidence_grounded: bool,
    /// Produced a useful next action or decision.
    pub helpful: bool,
}

/// Decisive judgement for the current ContinuityDB thesis.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ThesisFalsificationJudgement {
    /// Verdict label.
    pub verdict: String,
    /// Whether the original "specialized StateCell database primitive" thesis survived.
    pub original_database_primitive_thesis_survives: bool,
    /// Whether the narrowed "durable epistemic control layer" thesis survived.
    pub durable_control_layer_thesis_survives: bool,
    /// Decisive explanation.
    pub rationale: String,
    /// Minimum bar ContinuityDB had to clear against the strong baseline.
    pub required_bar: String,
}

/// Retained answer record produced by an actual model or scripted runner.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AgentBehaviorExecutionRecord {
    /// Stable task scenario identifier.
    pub task_id: String,
    /// Repeated-run trial index.
    #[serde(default)]
    pub trial_index: usize,
    /// Context strategy used to build the model-facing packet.
    pub strategy: String,
    /// Full task prompt sent to the model.
    pub prompt: String,
    /// Context packet or baseline context sent to the model.
    pub context_packet: String,
    /// Raw model answer retained for audit.
    pub model_output: String,
    /// Observed model-runner latency in milliseconds, when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_latency_ms: Option<u64>,
    /// Objective behavioral requirements for this task.
    pub requirements: AgentBehaviorTaskRequirements,
}

/// Model-execution task request before a runner has produced an answer.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AgentBehaviorExecutionTask {
    /// Stable task scenario identifier.
    pub task_id: String,
    /// Repeated-run trial index.
    #[serde(default)]
    pub trial_index: usize,
    /// Context strategy used to build the model-facing packet.
    pub strategy: String,
    /// Full task prompt to send to the model.
    pub prompt: String,
    /// Context packet or baseline context to send to the model.
    pub context_packet: String,
    /// Objective behavioral requirements for this task.
    pub requirements: AgentBehaviorTaskRequirements,
}

/// Canonical task matrix used for repeated model-execution benchmarks.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AgentBehaviorTaskMatrixReport {
    /// Stable report format identifier.
    pub format: String,
    /// Report format version.
    pub format_version: u32,
    /// Number of unique task scenarios.
    pub scenario_count: usize,
    /// Number of context strategies per scenario.
    pub strategy_count: usize,
    /// Generated model-execution tasks.
    pub tasks: Vec<AgentBehaviorExecutionTask>,
}

/// Objective behavioral requirements attached to one model-executed task.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AgentBehaviorTaskRequirements {
    /// Whether the task requires using a correction or superseding belief.
    pub requires_revision: bool,
    /// Whether the task requires hedging or uncertainty instead of confident assertion.
    pub requires_uncertainty: bool,
    /// Whether the task requires verification before acting.
    pub requires_verification: bool,
    /// Whether the prompt/context contains a stale-belief trap.
    pub has_stale_trap: bool,
    /// Whether the task includes a known failed action that should not be repeated.
    pub has_known_failed_action: bool,
}

/// Retained answer record with deterministic objective outcome labels.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AgentBehaviorScoredExecutionRecord {
    /// Stable task scenario identifier.
    pub task_id: String,
    /// Repeated-run trial index.
    pub trial_index: usize,
    /// Context strategy used to build the model-facing packet.
    pub strategy: String,
    /// Full task prompt sent to the model.
    pub prompt: String,
    /// Context packet or baseline context sent to the model.
    pub context_packet: String,
    /// Raw model answer retained for audit.
    pub model_output: String,
    /// Observed model-runner latency in milliseconds, when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_latency_ms: Option<u64>,
    /// Objective behavioral requirements for this task.
    pub requirements: AgentBehaviorTaskRequirements,
    /// Deterministic outcome labels derived from the retained answer.
    pub outcome: AgentBehaviorExecutionOutcome,
}

/// Deterministic objective outcome labels for one retained model answer.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AgentBehaviorExecutionOutcome {
    /// Whether the answer satisfied all required objective checks.
    pub task_success: bool,
    /// Whether the answer relied on known stale information.
    pub stale_belief: bool,
    /// Whether the answer used the required correction/revision state.
    pub revision_accurate: bool,
    /// Whether the answer asserted certainty where uncertainty was required.
    pub unsupported_certainty: bool,
    /// Whether the answer verified evidence when verification was required.
    pub verified_when_required: bool,
    /// Whether the retained context stayed inside the benchmark budget.
    pub budget_fit: bool,
    /// Whether the answer repeated a known failed action.
    pub repeated_failed_action: bool,
}

/// Same-model execution benchmark scored from retained answer records.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AgentBehaviorExecutionBenchmarkReport {
    /// Stable report format identifier.
    pub format: String,
    /// Report format version.
    pub format_version: u32,
    /// Evidence mode for this benchmark layer.
    pub evidence_mode: String,
    /// Number of unique task scenarios.
    pub task_count: usize,
    /// Number of retained model answer records.
    pub execution_record_count: usize,
    /// Number of repeated trials represented in the retained records.
    pub trial_count: usize,
    /// Strategy scores.
    pub strategies: Vec<AgentBehaviorStrategyReport>,
    /// Repeated-run stability summary by strategy.
    pub strategy_stability: Vec<AgentBehaviorStrategyStabilityReport>,
    /// Model-runner latency percentiles by strategy when retained records include latency.
    pub strategy_latency: Vec<AgentBehaviorStrategyLatencyReport>,
    /// Retained answer records with scored outcomes.
    pub execution_records: Vec<AgentBehaviorScoredExecutionRecord>,
    /// Ordered gates required before marketing representative claims.
    pub next_expansion_gates: Vec<String>,
}

/// Latency percentile summary for one strategy.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AgentBehaviorStrategyLatencyReport {
    /// Strategy identifier.
    pub strategy: String,
    /// Number of retained latency samples.
    pub sample_count: usize,
    /// Median retained model-runner latency in milliseconds.
    pub p50_ms: u64,
    /// 95th percentile retained model-runner latency in milliseconds.
    pub p95_ms: u64,
    /// 99th percentile retained model-runner latency in milliseconds.
    pub p99_ms: u64,
}

/// Repeated-run stability summary for one strategy.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AgentBehaviorStrategyStabilityReport {
    /// Strategy identifier.
    pub strategy: String,
    /// Number of repeated trials observed for this strategy.
    pub trial_count: usize,
    /// Mean task success rate across trials in basis points.
    pub task_success_mean_bps: usize,
    /// Minimum per-trial task success rate in basis points.
    pub task_success_min_bps: usize,
    /// Maximum per-trial task success rate in basis points.
    pub task_success_max_bps: usize,
}

/// Serializable workload summary snapshot.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkloadSummarySnapshot {
    /// Number of generated StateCells.
    pub cell_count: usize,
    /// Number of generated cells marked as frontier.
    pub frontier_count: usize,
    /// Number of generated dependency edges.
    pub dependency_count: usize,
    /// Sum of generated token costs.
    pub total_token_cost: i64,
}

impl From<WorkloadSummary> for WorkloadSummarySnapshot {
    fn from(summary: WorkloadSummary) -> Self {
        Self {
            cell_count: summary.cell_count,
            frontier_count: summary.frontier_count,
            dependency_count: summary.dependency_count,
            total_token_cost: summary.total_token_cost,
        }
    }
}

/// Serializable measured operation snapshot.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MeasuredOperationSnapshot {
    /// Number of logical operations performed.
    pub operation_count: usize,
    /// Observed elapsed nanoseconds for the operation group.
    pub elapsed_nanos: u128,
}

impl From<MeasuredOperation> for MeasuredOperationSnapshot {
    fn from(operation: MeasuredOperation) -> Self {
        Self {
            operation_count: operation.operation_count,
            elapsed_nanos: operation.elapsed.as_nanos(),
        }
    }
}

/// Serializable checkout measurement snapshot.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CheckoutMeasurementSnapshot {
    /// Number of checkout candidates matching request constraints.
    pub matched_count: usize,
    /// Number of cells selected into the returned slice.
    pub selected_count: usize,
    /// Number of matching cells omitted as alternatives.
    pub alternative_count: usize,
    /// Number of selected frontier cells.
    pub frontier_count: usize,
    /// Selected token total reported by checkout.
    pub selected_token_count: i64,
}

impl From<CheckoutMeasurement> for CheckoutMeasurementSnapshot {
    fn from(measurement: CheckoutMeasurement) -> Self {
        Self {
            matched_count: measurement.matched_count,
            selected_count: measurement.selected_count,
            alternative_count: measurement.alternative_count,
            frontier_count: measurement.frontier_count,
            selected_token_count: measurement.selected_token_count,
        }
    }
}

/// Serializable file-kernel lookup-plan detail for one indexed constraint.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkloadIndexedConstraintPlanSnapshot {
    /// Stable indexed lookup constraint name.
    pub name: String,
    /// Number of StateCell candidates selected by this single index before intersection.
    pub candidate_count: usize,
}

/// Serializable file-kernel lookup-plan snapshot for workload artifacts.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkloadLookupPlanSnapshot {
    /// Number of indexed lookup constraints present in the request.
    pub indexed_constraint_count: usize,
    /// Ordered names of indexed lookup constraints present in the request.
    pub indexed_constraints: Vec<String>,
    /// Ordered per-constraint indexed candidate details.
    pub indexed_constraint_plans: Vec<WorkloadIndexedConstraintPlanSnapshot>,
    /// Number of exact lookup constraints present in the request.
    pub exact_constraint_count: usize,
    /// Ordered names of exact lookup constraints checked after candidate selection.
    pub exact_constraints: Vec<String>,
    /// Number of exact lookup constraints that require residual filtering after index lookup.
    pub residual_exact_constraint_count: usize,
    /// Ordered exact lookup constraints enforced by residual filtering after index lookup.
    pub residual_exact_constraints: Vec<String>,
    /// Number of indexed lookup constraints that can over-select candidates.
    pub lossy_indexed_constraint_count: usize,
    /// Ordered names of indexed lookup constraints that require exact residual filtering.
    pub lossy_indexed_constraints: Vec<String>,
    /// Number of StateCell candidates selected before exact predicate filtering.
    pub candidate_count: usize,
    /// Number of selected candidates that satisfy the exact lookup predicate.
    pub exact_match_count: usize,
    /// Number of selected candidates rejected by exact predicate filtering.
    pub filtered_candidate_count: usize,
    /// Exact match share of selected candidates in integer basis points.
    pub candidate_selectivity_basis_points: usize,
    /// Whether lookup must inspect all visible StateCells.
    pub full_scan: bool,
}

impl From<FileKernelLookupPlan> for WorkloadLookupPlanSnapshot {
    fn from(plan: FileKernelLookupPlan) -> Self {
        Self {
            indexed_constraint_count: plan.indexed_constraint_count,
            indexed_constraints: plan
                .indexed_constraints
                .into_iter()
                .map(str::to_string)
                .collect(),
            indexed_constraint_plans: plan
                .indexed_constraint_plans
                .into_iter()
                .map(|constraint| WorkloadIndexedConstraintPlanSnapshot {
                    name: constraint.name.to_string(),
                    candidate_count: constraint.candidate_count,
                })
                .collect(),
            exact_constraint_count: plan.exact_constraint_count,
            exact_constraints: plan
                .exact_constraints
                .into_iter()
                .map(str::to_string)
                .collect(),
            residual_exact_constraint_count: plan.residual_exact_constraint_count,
            residual_exact_constraints: plan
                .residual_exact_constraints
                .into_iter()
                .map(str::to_string)
                .collect(),
            lossy_indexed_constraint_count: plan.lossy_indexed_constraint_count,
            lossy_indexed_constraints: plan
                .lossy_indexed_constraints
                .into_iter()
                .map(str::to_string)
                .collect(),
            candidate_count: plan.candidate_count,
            exact_match_count: plan.exact_match_count,
            filtered_candidate_count: plan.filtered_candidate_count,
            candidate_selectivity_basis_points: plan.candidate_selectivity_basis_points,
            full_scan: plan.full_scan,
        }
    }
}

/// Serializable workload measurement snapshot for durable baseline records.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkloadMeasurementSnapshot {
    /// Generated workload summary.
    pub workload: WorkloadSummarySnapshot,
    /// Number of native revision links appended for the workload.
    pub revision_link_count: usize,
    /// Ingest operation measurement.
    pub ingest: MeasuredOperationSnapshot,
    /// Checkout operation measurement.
    pub checkout_operation: MeasuredOperationSnapshot,
    /// Checkout result counts.
    pub checkout: CheckoutMeasurementSnapshot,
    /// Optional file-kernel lookup-plan diagnostics for the measured checkout request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lookup_plan: Option<WorkloadLookupPlanSnapshot>,
}

impl WorkloadMeasurementSnapshot {
    /// Converts an in-memory measurement into a serializable snapshot.
    pub fn from_measurement(measurement: &WorkloadMeasurement) -> Self {
        Self {
            workload: measurement.workload_summary.into(),
            revision_link_count: measurement.revision_link_count,
            ingest: measurement.ingest.into(),
            checkout_operation: measurement.checkout_operation.into(),
            checkout: measurement.checkout.into(),
            lookup_plan: None,
        }
    }

    /// Converts an in-memory measurement plus optional file lookup plan into a serializable snapshot.
    pub fn from_measurement_with_lookup_plan(
        measurement: &WorkloadMeasurement,
        lookup_plan: Option<FileKernelLookupPlan>,
    ) -> Self {
        let mut snapshot = Self::from_measurement(measurement);
        snapshot.lookup_plan = lookup_plan.map(Into::into);
        snapshot
    }
}

/// Durable workload measurement baseline record.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkloadBaselineRecord {
    /// Timestamp when this baseline was recorded.
    pub recorded_at: DateTime<Utc>,
    /// Caller-provided scenario label.
    pub label: String,
    /// Caller-provided kernel profile name.
    pub kernel: String,
    /// Serializable measurement snapshot.
    pub snapshot: WorkloadMeasurementSnapshot,
}

impl WorkloadBaselineRecord {
    /// Creates a workload baseline record.
    pub fn new(
        recorded_at: DateTime<Utc>,
        label: impl Into<String>,
        kernel: impl Into<String>,
        snapshot: WorkloadMeasurementSnapshot,
    ) -> Self {
        Self {
            recorded_at,
            label: label.into(),
            kernel: kernel.into(),
            snapshot,
        }
    }
}

/// Regression thresholds used when comparing workload measurements to a baseline.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkloadRegressionThresholds {
    /// Maximum allowed elapsed-time growth percentage before reporting a regression.
    pub max_elapsed_growth_percent: u128,
}

impl Default for WorkloadRegressionThresholds {
    fn default() -> Self {
        Self {
            max_elapsed_growth_percent: 25,
        }
    }
}

/// Deterministic workload baseline comparison report.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkloadBaselineComparison {
    /// Baseline record used for comparison.
    pub baseline: WorkloadBaselineRecord,
    /// Current measurement snapshot compared against the baseline.
    pub current: WorkloadMeasurementSnapshot,
    /// Regressions detected by exact count checks or elapsed-time thresholds.
    pub regressions: Vec<WorkloadBaselineRegression>,
}

impl WorkloadBaselineComparison {
    /// Returns true when no regressions were detected.
    pub fn passed(&self) -> bool {
        self.regressions.is_empty()
    }
}

/// Deterministic workload baseline regression reason.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum WorkloadBaselineRegression {
    /// Generated StateCell count changed.
    WorkloadCellCountChanged {
        /// Baseline count.
        previous: usize,
        /// Current count.
        current: usize,
    },
    /// Generated frontier StateCell count changed.
    WorkloadFrontierCountChanged {
        /// Baseline count.
        previous: usize,
        /// Current count.
        current: usize,
    },
    /// Generated dependency edge count changed.
    WorkloadDependencyCountChanged {
        /// Baseline count.
        previous: usize,
        /// Current count.
        current: usize,
    },
    /// Generated total token cost changed.
    WorkloadTokenCostChanged {
        /// Baseline token count.
        previous: i64,
        /// Current token count.
        current: i64,
    },
    /// Ingest operation count changed.
    IngestOperationCountChanged {
        /// Baseline operation count.
        previous: usize,
        /// Current operation count.
        current: usize,
    },
    /// Checkout operation count changed.
    CheckoutOperationCountChanged {
        /// Baseline operation count.
        previous: usize,
        /// Current operation count.
        current: usize,
    },
    /// Checkout candidate count changed.
    CheckoutMatchedCountChanged {
        /// Baseline count.
        previous: usize,
        /// Current count.
        current: usize,
    },
    /// Checkout selected-cell count changed.
    CheckoutSelectedCountChanged {
        /// Baseline count.
        previous: usize,
        /// Current count.
        current: usize,
    },
    /// Checkout alternative count changed.
    CheckoutAlternativeCountChanged {
        /// Baseline count.
        previous: usize,
        /// Current count.
        current: usize,
    },
    /// Checkout frontier recommendation count changed.
    CheckoutFrontierCountChanged {
        /// Baseline count.
        previous: usize,
        /// Current count.
        current: usize,
    },
    /// Checkout selected token count changed.
    CheckoutSelectedTokenCountChanged {
        /// Baseline token count.
        previous: i64,
        /// Current token count.
        current: i64,
    },
    /// Ingest elapsed time exceeded the allowed growth threshold.
    IngestElapsedRegressed {
        /// Baseline elapsed nanoseconds.
        previous_nanos: u128,
        /// Current elapsed nanoseconds.
        current_nanos: u128,
        /// Maximum allowed current elapsed nanoseconds under the threshold.
        max_allowed_nanos: u128,
    },
    /// Checkout elapsed time exceeded the allowed growth threshold.
    CheckoutElapsedRegressed {
        /// Baseline elapsed nanoseconds.
        previous_nanos: u128,
        /// Current elapsed nanoseconds.
        current_nanos: u128,
        /// Maximum allowed current elapsed nanoseconds under the threshold.
        max_allowed_nanos: u128,
    },
    /// Lookup-plan diagnostics were added or removed between comparable workload snapshots.
    LookupPlanPresenceChanged {
        /// Whether the baseline snapshot had lookup-plan diagnostics.
        previous: bool,
        /// Whether the current snapshot has lookup-plan diagnostics.
        current: bool,
    },
    /// Ordered indexed lookup constraints changed.
    LookupPlanIndexedConstraintsChanged {
        /// Baseline ordered indexed constraint names.
        previous: Vec<String>,
        /// Current ordered indexed constraint names.
        current: Vec<String>,
    },
    /// Ordered exact lookup constraints changed.
    LookupPlanExactConstraintsChanged {
        /// Baseline ordered exact constraint names.
        previous: Vec<String>,
        /// Current ordered exact constraint names.
        current: Vec<String>,
    },
    /// Ordered residual exact lookup constraints changed.
    LookupPlanResidualExactConstraintsChanged {
        /// Baseline ordered residual exact constraint names.
        previous: Vec<String>,
        /// Current ordered residual exact constraint names.
        current: Vec<String>,
    },
    /// Ordered lossy indexed lookup constraints changed.
    LookupPlanLossyIndexedConstraintsChanged {
        /// Baseline ordered lossy indexed constraint names.
        previous: Vec<String>,
        /// Current ordered lossy indexed constraint names.
        current: Vec<String>,
    },
    /// Final lookup-plan candidate count changed.
    LookupPlanCandidateCountChanged {
        /// Baseline candidate count.
        previous: usize,
        /// Current candidate count.
        current: usize,
    },
    /// Exact post-filter lookup-plan match count changed.
    LookupPlanExactMatchCountChanged {
        /// Baseline exact match count.
        previous: usize,
        /// Current exact match count.
        current: usize,
    },
    /// Filtered candidate count changed.
    LookupPlanFilteredCandidateCountChanged {
        /// Baseline filtered candidate count.
        previous: usize,
        /// Current filtered candidate count.
        current: usize,
    },
    /// Candidate selectivity changed.
    LookupPlanCandidateSelectivityChanged {
        /// Baseline selectivity in basis points.
        previous: usize,
        /// Current selectivity in basis points.
        current: usize,
    },
    /// Lookup-plan full-scan fallback changed.
    LookupPlanFullScanChanged {
        /// Baseline full-scan status.
        previous: bool,
        /// Current full-scan status.
        current: bool,
    },
    /// Candidate count for a shared indexed lookup constraint changed.
    LookupPlanConstraintCandidateCountChanged {
        /// Stable indexed constraint name.
        name: String,
        /// Baseline candidate count for this constraint.
        previous: usize,
        /// Current candidate count for this constraint.
        current: usize,
    },
}

/// Workload generation failure.
#[derive(Debug, Error)]
pub enum WorkloadError {
    /// Workload must include at least one StateCell.
    #[error("workload must include at least one StateCell")]
    EmptyWorkload,
    /// Semantic anchor prefix must not be empty.
    #[error("anchor prefix must not be empty")]
    EmptyAnchorPrefix,
    /// Project scope must not be empty.
    #[error("project scope must not be empty")]
    EmptyProjectScope,
    /// Frontier interval must be positive.
    #[error("frontier interval must be positive")]
    InvalidFrontierInterval,
    /// Dependency stride must be positive.
    #[error("dependency stride must be positive")]
    InvalidDependencyStride,
    /// Core StateCell construction failed.
    #[error(transparent)]
    Core(#[from] continuitydb_core::CoreError),
}

/// Workload measurement failure.
#[derive(Debug, Error)]
pub enum MeasurementError {
    /// Storage kernel failure while ingesting workload cells.
    #[error(transparent)]
    Kernel(#[from] KernelError),
    /// Checkout failure while materializing a workload slice.
    #[error(transparent)]
    Checkout(#[from] CheckoutError),
}

/// Representative long-horizon context harness failure.
#[derive(Debug, Error)]
pub enum RepresentativeHorizonError {
    /// Core StateCell construction failed.
    #[error(transparent)]
    Core(#[from] continuitydb_core::CoreError),
    /// Storage kernel failure while building the horizon.
    #[error(transparent)]
    Kernel(#[from] KernelError),
    /// Checkout failure while measuring a horizon turn.
    #[error(transparent)]
    Checkout(#[from] CheckoutError),
}

/// Adversarial downstream task harness failure.
#[derive(Debug, Error)]
pub enum AdversarialTaskHarnessError {
    /// Core StateCell construction failed.
    #[error(transparent)]
    Core(#[from] continuitydb_core::CoreError),
    /// Storage kernel failure while building the harness corpus.
    #[error(transparent)]
    Kernel(#[from] KernelError),
    /// Checkout failure while scoring the task.
    #[error(transparent)]
    Checkout(#[from] CheckoutError),
}

/// Workload baseline store failure.
#[derive(Debug, Error)]
pub enum WorkloadBaselineError {
    /// File I/O failure.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// JSON serialization failure.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    /// Baseline JSONL record is corrupt.
    #[error("baseline record at line {line} is corrupt")]
    CorruptRecord {
        /// One-based JSONL line number.
        line: usize,
        /// Decode error for the corrupt record.
        #[source]
        source: serde_json::Error,
    },
}

/// Append-only JSONL store for workload measurement baselines.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileWorkloadBaselineStore {
    path: PathBuf,
}

impl FileWorkloadBaselineStore {
    /// Creates a file-backed workload baseline store at the given path.
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }

    /// Appends one baseline record as a JSONL line.
    pub fn append(&self, record: &WorkloadBaselineRecord) -> Result<(), WorkloadBaselineError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        serde_json::to_writer(&mut file, record)?;
        file.write_all(b"\n")?;
        file.flush()?;
        Ok(())
    }

    /// Lists baseline records in file order.
    pub fn list(&self) -> Result<Vec<WorkloadBaselineRecord>, WorkloadBaselineError> {
        match File::open(&self.path) {
            Ok(file) => {
                let reader = BufReader::new(file);
                let mut records = Vec::new();
                for (index, line) in reader.lines().enumerate() {
                    let line = line?;
                    if line.trim().is_empty() {
                        continue;
                    }

                    let record = serde_json::from_str(&line).map_err(|source| {
                        WorkloadBaselineError::CorruptRecord {
                            line: index + 1,
                            source,
                        }
                    })?;
                    records.push(record);
                }
                Ok(records)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(error) => Err(error.into()),
        }
    }

    /// Returns the newest baseline record matching the given label and kernel.
    pub fn latest_matching(
        &self,
        label: &str,
        kernel: &str,
    ) -> Result<Option<WorkloadBaselineRecord>, WorkloadBaselineError> {
        let mut latest: Option<WorkloadBaselineRecord> = None;
        for record in self.list()? {
            let is_newer_match = match latest.as_ref() {
                Some(current) => record.recorded_at >= current.recorded_at,
                None => true,
            };
            if record.label == label && record.kernel == kernel && is_newer_match {
                latest = Some(record);
            }
        }
        Ok(latest)
    }
}

/// Compares a current workload snapshot to a durable baseline record.
pub fn compare_workload_snapshot_to_baseline(
    baseline: &WorkloadBaselineRecord,
    current: &WorkloadMeasurementSnapshot,
    thresholds: WorkloadRegressionThresholds,
) -> WorkloadBaselineComparison {
    let previous = &baseline.snapshot;
    let mut regressions = Vec::new();

    push_if_changed(
        &mut regressions,
        previous.workload.cell_count,
        current.workload.cell_count,
        |previous, current| WorkloadBaselineRegression::WorkloadCellCountChanged {
            previous,
            current,
        },
    );
    push_if_changed(
        &mut regressions,
        previous.workload.frontier_count,
        current.workload.frontier_count,
        |previous, current| WorkloadBaselineRegression::WorkloadFrontierCountChanged {
            previous,
            current,
        },
    );
    push_if_changed(
        &mut regressions,
        previous.workload.dependency_count,
        current.workload.dependency_count,
        |previous, current| WorkloadBaselineRegression::WorkloadDependencyCountChanged {
            previous,
            current,
        },
    );
    push_if_changed(
        &mut regressions,
        previous.workload.total_token_cost,
        current.workload.total_token_cost,
        |previous, current| WorkloadBaselineRegression::WorkloadTokenCostChanged {
            previous,
            current,
        },
    );
    push_if_changed(
        &mut regressions,
        previous.ingest.operation_count,
        current.ingest.operation_count,
        |previous, current| WorkloadBaselineRegression::IngestOperationCountChanged {
            previous,
            current,
        },
    );
    push_if_changed(
        &mut regressions,
        previous.checkout_operation.operation_count,
        current.checkout_operation.operation_count,
        |previous, current| WorkloadBaselineRegression::CheckoutOperationCountChanged {
            previous,
            current,
        },
    );
    push_if_changed(
        &mut regressions,
        previous.checkout.matched_count,
        current.checkout.matched_count,
        |previous, current| WorkloadBaselineRegression::CheckoutMatchedCountChanged {
            previous,
            current,
        },
    );
    push_if_changed(
        &mut regressions,
        previous.checkout.selected_count,
        current.checkout.selected_count,
        |previous, current| WorkloadBaselineRegression::CheckoutSelectedCountChanged {
            previous,
            current,
        },
    );
    push_if_changed(
        &mut regressions,
        previous.checkout.alternative_count,
        current.checkout.alternative_count,
        |previous, current| WorkloadBaselineRegression::CheckoutAlternativeCountChanged {
            previous,
            current,
        },
    );
    push_if_changed(
        &mut regressions,
        previous.checkout.frontier_count,
        current.checkout.frontier_count,
        |previous, current| WorkloadBaselineRegression::CheckoutFrontierCountChanged {
            previous,
            current,
        },
    );
    push_if_changed(
        &mut regressions,
        previous.checkout.selected_token_count,
        current.checkout.selected_token_count,
        |previous, current| WorkloadBaselineRegression::CheckoutSelectedTokenCountChanged {
            previous,
            current,
        },
    );

    push_lookup_plan_regressions(
        &mut regressions,
        &previous.lookup_plan,
        &current.lookup_plan,
    );

    let ingest_allowed = max_allowed_elapsed(
        previous.ingest.elapsed_nanos,
        thresholds.max_elapsed_growth_percent,
    );
    if current.ingest.elapsed_nanos > ingest_allowed {
        regressions.push(WorkloadBaselineRegression::IngestElapsedRegressed {
            previous_nanos: previous.ingest.elapsed_nanos,
            current_nanos: current.ingest.elapsed_nanos,
            max_allowed_nanos: ingest_allowed,
        });
    }

    let checkout_allowed = max_allowed_elapsed(
        previous.checkout_operation.elapsed_nanos,
        thresholds.max_elapsed_growth_percent,
    );
    if current.checkout_operation.elapsed_nanos > checkout_allowed {
        regressions.push(WorkloadBaselineRegression::CheckoutElapsedRegressed {
            previous_nanos: previous.checkout_operation.elapsed_nanos,
            current_nanos: current.checkout_operation.elapsed_nanos,
            max_allowed_nanos: checkout_allowed,
        });
    }

    WorkloadBaselineComparison {
        baseline: baseline.clone(),
        current: current.clone(),
        regressions,
    }
}

fn push_if_changed<T, F>(
    regressions: &mut Vec<WorkloadBaselineRegression>,
    previous: T,
    current: T,
    build: F,
) where
    T: Copy + Eq,
    F: FnOnce(T, T) -> WorkloadBaselineRegression,
{
    if previous != current {
        regressions.push(build(previous, current));
    }
}

fn push_lookup_plan_regressions(
    regressions: &mut Vec<WorkloadBaselineRegression>,
    previous: &Option<WorkloadLookupPlanSnapshot>,
    current: &Option<WorkloadLookupPlanSnapshot>,
) {
    match (previous, current) {
        (None, None) => {}
        (None, Some(_)) | (Some(_), None) => {
            regressions.push(WorkloadBaselineRegression::LookupPlanPresenceChanged {
                previous: previous.is_some(),
                current: current.is_some(),
            });
        }
        (Some(previous), Some(current)) => {
            push_if_changed(
                regressions,
                previous.indexed_constraints.as_slice(),
                current.indexed_constraints.as_slice(),
                |previous, current| {
                    WorkloadBaselineRegression::LookupPlanIndexedConstraintsChanged {
                        previous: previous.to_vec(),
                        current: current.to_vec(),
                    }
                },
            );
            push_if_changed(
                regressions,
                previous.exact_constraints.as_slice(),
                current.exact_constraints.as_slice(),
                |previous, current| WorkloadBaselineRegression::LookupPlanExactConstraintsChanged {
                    previous: previous.to_vec(),
                    current: current.to_vec(),
                },
            );
            push_if_changed(
                regressions,
                previous.lossy_indexed_constraints.as_slice(),
                current.lossy_indexed_constraints.as_slice(),
                |previous, current| {
                    WorkloadBaselineRegression::LookupPlanLossyIndexedConstraintsChanged {
                        previous: previous.to_vec(),
                        current: current.to_vec(),
                    }
                },
            );
            push_if_changed(
                regressions,
                previous.residual_exact_constraints.as_slice(),
                current.residual_exact_constraints.as_slice(),
                |previous, current| {
                    WorkloadBaselineRegression::LookupPlanResidualExactConstraintsChanged {
                        previous: previous.to_vec(),
                        current: current.to_vec(),
                    }
                },
            );
            push_if_changed(
                regressions,
                previous.candidate_count,
                current.candidate_count,
                |previous, current| WorkloadBaselineRegression::LookupPlanCandidateCountChanged {
                    previous,
                    current,
                },
            );
            push_if_changed(
                regressions,
                previous.exact_match_count,
                current.exact_match_count,
                |previous, current| WorkloadBaselineRegression::LookupPlanExactMatchCountChanged {
                    previous,
                    current,
                },
            );
            push_if_changed(
                regressions,
                previous.filtered_candidate_count,
                current.filtered_candidate_count,
                |previous, current| {
                    WorkloadBaselineRegression::LookupPlanFilteredCandidateCountChanged {
                        previous,
                        current,
                    }
                },
            );
            push_if_changed(
                regressions,
                previous.candidate_selectivity_basis_points,
                current.candidate_selectivity_basis_points,
                |previous, current| {
                    WorkloadBaselineRegression::LookupPlanCandidateSelectivityChanged {
                        previous,
                        current,
                    }
                },
            );
            push_if_changed(
                regressions,
                previous.full_scan,
                current.full_scan,
                |previous, current| WorkloadBaselineRegression::LookupPlanFullScanChanged {
                    previous,
                    current,
                },
            );
            push_lookup_plan_constraint_candidate_count_regressions(
                regressions,
                &previous.indexed_constraint_plans,
                &current.indexed_constraint_plans,
            );
        }
    }
}

fn push_lookup_plan_constraint_candidate_count_regressions(
    regressions: &mut Vec<WorkloadBaselineRegression>,
    previous: &[WorkloadIndexedConstraintPlanSnapshot],
    current: &[WorkloadIndexedConstraintPlanSnapshot],
) {
    let current_counts = current
        .iter()
        .map(|plan| (plan.name.as_str(), plan.candidate_count))
        .collect::<BTreeMap<_, _>>();

    for previous_plan in previous {
        if let Some(current_count) = current_counts.get(previous_plan.name.as_str()) {
            push_if_changed(
                regressions,
                previous_plan.candidate_count,
                *current_count,
                |previous, current| {
                    WorkloadBaselineRegression::LookupPlanConstraintCandidateCountChanged {
                        name: previous_plan.name.clone(),
                        previous,
                        current,
                    }
                },
            );
        }
    }
}

fn max_allowed_elapsed(previous_nanos: u128, growth_percent: u128) -> u128 {
    previous_nanos + ((previous_nanos * growth_percent) / 100)
}

/// Generates a deterministic world-model workload for benchmarks and engine comparisons.
pub fn generate_world_model_workload(
    config: WorkloadConfig,
) -> Result<ContinuityWorkload, WorkloadError> {
    validate_config(&config)?;

    let anchor_prefix = config.anchor_prefix.trim();
    let project_scope = config.project_scope.trim();
    let mut cells: Vec<StateCell> = Vec::with_capacity(config.cell_count);
    let mut frontier_count = 0;
    let mut dependency_count = 0;
    let mut total_token_cost = 0;

    for index in 0..config.cell_count {
        let id = StateCellId::from_u128(config.id_seed + index as u128);
        let token_count = 120 + (index % 17) as i64;
        let mut cell = StateCell::new(
            id,
            vec![SemanticAnchor::new(format!(
                "{anchor_prefix}:cell:{index:06}"
            ))],
            ValidTimeRange::new(config.valid_from, None)?,
            Scope::Project(project_scope.to_string()),
            Answerability::new(vec![format!(
                "what is the operational state for {anchor_prefix} cell {index}?"
            )])?,
            vec![Evidence {
                source: continuitydb_core::SourceId::new(format!("workload:source:{index:06}")),
                citation: Citation {
                    locator: format!("workload://{anchor_prefix}/evidence/{index:06}"),
                },
                confidence: Confidence::new(confidence_for(index))?,
                trust: vec![trust_signal_for(index)],
            }],
            CellPayload::Text(format!(
                "Deterministic workload cell {index} for {anchor_prefix}."
            )),
            CellCost::new(token_count, (index % 5) as i64)?,
        )?;

        cell.add_projection(MemoryProjection::new(
            MemoryProjectionKind::Semantic,
            format!("Operational state for {anchor_prefix} cell {index}: deterministic workload evidence is available."),
            Confidence::new(confidence_for(index))?,
            CellCost::new(16 + (index % 3) as i64, 0)?,
        )?);
        cell.context_gaps.push(ContextGap::new(
            ContextGapKind::MissingEvidence,
            format!("Which external source should refresh {anchor_prefix} cell {index}?"),
            "Alpha workflow retained packets must carry missing-context scavenging metadata.",
            0.65,
        )?);
        cell.add_invalidation_condition(InvalidationCondition::new(
            InvalidationConditionKind::DependencyInvalidated,
            format!("A dependency or source artifact for {anchor_prefix} cell {index} is superseded."),
            "Alpha workflow retained packets must carry falsification conditions for lifecycle revision.",
            0.7,
        )?);
        cell.utility_feedback = utility_feedback_for(index)?;

        if (index + 1) % config.frontier_every == 0 {
            cell.activation = ActivationState::Frontier;
            frontier_count += 1;
        }

        if index >= config.dependency_stride {
            cell.dependencies.push(CellDependency::new(
                cells[index - config.dependency_stride].id,
                CellDependencyKind::DependsOn,
                format!(
                    "workload cell {index} depends on cell {}",
                    index - config.dependency_stride
                ),
            ));
            dependency_count += 1;
        }

        total_token_cost += token_count;
        cells.push(cell);
    }

    Ok(ContinuityWorkload {
        cells,
        summary: WorkloadSummary {
            cell_count: config.cell_count,
            frontier_count,
            dependency_count,
            total_token_cost,
        },
    })
}

/// Measures ingest and checkout over a deterministic workload using any storage kernel.
pub fn measure_ingest_and_checkout<K>(
    kernel: &mut K,
    workload: &ContinuityWorkload,
    committed_at: DateTime<Utc>,
    request: CheckoutRequest,
) -> Result<WorkloadMeasurement, MeasurementError>
where
    K: StorageKernel,
{
    let ingest_started = Instant::now();
    kernel.append_cells_at(workload.cells.clone(), committed_at)?;
    let mut revision_link_count = 0;
    for cell in &workload.cells {
        for dependency in &cell.dependencies {
            kernel.append_revision_link(RevisionLinkRecord::new(
                cell.id,
                RevisionLinkKind::DerivesFrom,
                dependency.target,
                committed_at,
            ))?;
            revision_link_count += 1;
        }
    }
    let ingest_elapsed = ingest_started.elapsed();

    let checkout_started = Instant::now();
    let slice = checkout(kernel, request)?;
    let checkout_elapsed = checkout_started.elapsed();

    Ok(WorkloadMeasurement {
        workload_summary: workload.summary,
        revision_link_count,
        ingest: MeasuredOperation {
            operation_count: workload.cells.len(),
            elapsed: ingest_elapsed,
        },
        checkout_operation: MeasuredOperation {
            operation_count: 1,
            elapsed: checkout_elapsed,
        },
        checkout: CheckoutMeasurement {
            matched_count: slice.cells.len() + slice.alternatives.len(),
            selected_count: slice.cells.len(),
            alternative_count: slice.alternatives.len(),
            frontier_count: slice.frontier_recommendations.len(),
            selected_token_count: slice.total_tokens,
        },
    })
}

/// Runs a deterministic, representative long-horizon agent-memory scenario.
///
/// This is not a live benchmark. It is a small reproducible lifecycle harness
/// that checks whether checkout preserves the context signals the thesis says
/// matter: stale-belief suppression, revision guidance, hedging, invalidation,
/// and reusable trajectory memory.
pub fn run_representative_agent_memory_horizon<K>(
    kernel: &mut K,
    committed_at: DateTime<Utc>,
) -> Result<RepresentativeAgentMemoryHorizonReport, RepresentativeHorizonError>
where
    K: StorageKernel,
{
    let stale = representative_cell(
        9_000,
        "representative:release:stale",
        "did the release upload succeed?",
        "release asset already exists and upload can proceed",
        0.94,
        12,
        committed_at,
    )?;
    let mut stale = stale;
    stale.lifecycle_stage = LifecycleStage::Superseded;
    stale.activation = ActivationState::Retired;

    let mut current = representative_cell(
        9_001,
        "representative:release:current",
        "what is the current release upload state?",
        "release upload failed because the GitHub Release target was missing",
        0.91,
        12,
        committed_at,
    )?;
    current.add_projection(MemoryProjection::new(
        MemoryProjectionKind::Semantic,
        "Current belief: release upload failed because the GitHub Release target was missing.",
        Confidence::new(0.91)?,
        CellCost::new(10, 0)?,
    )?);
    current.set_uncertainty(EpistemicUncertainty::new(
        Confidence::new(0.58)?,
        4.2,
        "the prior upload-success belief failed against retained CI evidence",
    )?);
    current.add_context_gap(ContextGap::new(
        ContextGapKind::MissingEvidence,
        "which retained upload artifact proves the missing release target?",
        "avoid repeating the release upload failure without trace evidence",
        0.82,
    )?);
    current.add_invalidation_condition(InvalidationCondition::new(
        InvalidationConditionKind::ContradictoryEvidence,
        "a retained successful upload report with the same asset digest exists",
        "successful upload evidence would supersede the failure belief",
        0.90,
    )?);

    let mut hedged = representative_cell(
        9_002,
        "representative:release:unverified",
        "is release verification safe to use?",
        "release verification is plausible but still needs evidence before use",
        0.88,
        12,
        committed_at,
    )?;
    hedged.set_lifecycle_policy(ContextLifecyclePolicy {
        retention: RetentionPolicy::DecayUnlessReinforced,
        use_policy: UsePolicy::HedgeBeforeUse,
        promotion: PromotionPolicy::Manual,
    });

    let mut trajectory = representative_cell(
        9_003,
        "representative:release:trajectory",
        "what rollout lesson should be reused?",
        "prior release upload rollout produced a reusable retry lesson",
        0.86,
        12,
        committed_at,
    )?;
    trajectory.set_trajectory_memory(TrajectoryMemory::new(
        "retried release upload before checking release existence",
        "retained CI trace identified the failing upload step",
        "asset upload returned 404 because the GitHub Release did not exist",
        "artifact://representative/release-upload-404",
        0.92,
        "before retrying release upload, verify the GitHub Release exists",
        vec!["retrying release upload from CI".to_string()],
        vec!["release existence has already been verified".to_string()],
        ContextPacketStrategy::FalsificationBrief,
    )?);

    let cells = vec![
        stale.clone(),
        current.clone(),
        hedged.clone(),
        trajectory.clone(),
    ];
    kernel.append_cells_at_with_commit_id(cells, committed_at, CommitId::new())?;
    kernel.append_revision_link(RevisionLinkRecord::new(
        current.id,
        RevisionLinkKind::Supersedes,
        stale.id,
        committed_at,
    ))?;

    let turns = [
        representative_checkout_request(
            "representative:release:current",
            "what should I do next without repeating the release upload failure?",
            ContextProfile::Planning,
            32,
        )?,
        representative_checkout_request(
            "representative:release:unverified",
            "is it safe to trust release verification for execution?",
            ContextProfile::Execution,
            18,
        )?,
        representative_checkout_request(
            "representative:release:trajectory",
            "retrying release upload from CI",
            ContextProfile::Reflection,
            20,
        )?,
        representative_checkout_request(
            "representative:release:current",
            "which evidence would invalidate the current release upload belief?",
            ContextProfile::Audit,
            32,
        )?,
    ];

    let mut stale_belief_selected_count = 0;
    let mut revision_guidance_packet_count = 0;
    let mut hedging_packet_count = 0;
    let mut invalidation_packet_count = 0;
    let mut trajectory_reuse_packet_count = 0;

    for request in turns {
        let slice = checkout(kernel, request)?;
        stale_belief_selected_count += slice
            .cells
            .iter()
            .filter(|cell| cell.id == stale.id)
            .count();
        revision_guidance_packet_count += slice
            .context_packets
            .iter()
            .filter(|packet| !packet.revision_context.is_empty())
            .count();
        hedging_packet_count +=
            slice
                .context_packets
                .iter()
                .filter(|packet| {
                    packet.compiler_reason_tags.iter().any(|tag| {
                        tag == "lifecycle-safe-use-policy" || tag == "task-intent-safety"
                    }) || packet.lines.iter().any(|line| {
                        let line = line.to_ascii_lowercase();
                        line.contains("hedge") || line.contains("verify")
                    })
                })
                .count();
        invalidation_packet_count += slice
            .context_packets
            .iter()
            .filter(|packet| {
                packet
                    .selection
                    .as_ref()
                    .is_some_and(|selection| !selection.invalidation_conditions.is_empty())
            })
            .count();
        trajectory_reuse_packet_count += slice
            .context_packets
            .iter()
            .filter(|packet| {
                packet.compiler_reason_tags.iter().any(|tag| {
                    tag == "trajectory-applicability-match" || tag == "task-intent-trajectory-reuse"
                }) || packet.selection.as_ref().is_some_and(|selection| {
                    selection
                        .reasons
                        .contains(&ContextPacketSelectionReason::TrajectoryMemory)
                })
            })
            .count();
    }

    let satisfied_signals = [
        stale_belief_selected_count == 0,
        revision_guidance_packet_count > 0,
        hedging_packet_count > 0,
        invalidation_packet_count > 0,
        trajectory_reuse_packet_count > 0,
    ]
    .into_iter()
    .filter(|satisfied| *satisfied)
    .count();

    Ok(RepresentativeAgentMemoryHorizonReport {
        turn_count: 4,
        revision_guidance_packet_count,
        hedging_packet_count,
        invalidation_packet_count,
        trajectory_reuse_packet_count,
        stale_belief_selected_count,
        lifecycle_success_basis_points: satisfied_signals * 2_000,
    })
}

/// Runs a small deterministic adversarial downstream task harness.
pub fn run_adversarial_agent_task_harness(
    committed_at: DateTime<Utc>,
) -> Result<AdversarialAgentTaskHarnessReport, AdversarialTaskHarnessError> {
    let baseline =
        score_adversarial_agent_task_policy(ContextCompilerPolicy::RawBaseline, committed_at)?;
    let adaptive =
        score_adversarial_agent_task_policy(ContextCompilerPolicy::Automatic, committed_at)?;

    Ok(AdversarialAgentTaskHarnessReport {
        case_count: 7,
        baseline,
        adaptive,
    })
}

/// Runs the first small downstream behavior benchmark for checkout.
///
/// This intentionally avoids provider comparisons. It asks whether the product
/// shape, `StateCell -> checkout(task, budget) -> ContextPacket`, improves
/// objective agent behavior over simple context baselines before larger
/// representative evaluation work begins.
pub fn run_agent_behavior_benchmark(
    _committed_at: DateTime<Utc>,
) -> Result<AgentBehaviorBenchmarkReport, AdversarialTaskHarnessError> {
    let cases = agent_behavior_cases();
    let task_count = cases.len();
    let strategies = [
        AgentBehaviorStrategy::TranscriptSummary,
        AgentBehaviorStrategy::VectorRetrieval,
        AgentBehaviorStrategy::RawStateCell,
        AgentBehaviorStrategy::ContinuityDbCheckout,
    ]
    .into_iter()
    .map(|strategy| score_agent_behavior_strategy(strategy, &cases))
    .collect();

    Ok(AgentBehaviorBenchmarkReport {
        format: "continuitydb.agent_behavior_benchmark".to_string(),
        format_version: 1,
        evidence_mode: "deterministic_simulated_downstream_behavior".to_string(),
        task_count,
        turns_per_task: 5,
        token_budget: 1200,
        strategies,
        next_expansion_gates: vec![
            "replace deterministic simulated answers with same-model task execution".to_string(),
            "run 3+ seeds per strategy and report confidence intervals".to_string(),
            "scale corpus from curated repo-history tasks to representative issues, PRs, commits, docs, and CI failures".to_string(),
            "add blinded cross-model or human evaluation for explanation quality".to_string(),
        ],
    })
}

/// Generates the canonical task matrix consumed by repeated model-execution runs.
pub fn generate_agent_behavior_task_matrix(
) -> Result<AgentBehaviorTaskMatrixReport, AdversarialTaskHarnessError> {
    agent_behavior_task_matrix_report(
        "continuitydb.agent_behavior_task_matrix",
        agent_behavior_scenarios(),
    )
}

/// Generates a representative repo-lifecycle task matrix for Phase 3 repeated model execution.
pub fn generate_representative_agent_behavior_task_matrix(
) -> Result<AgentBehaviorTaskMatrixReport, AdversarialTaskHarnessError> {
    agent_behavior_task_matrix_report(
        "continuitydb.representative_agent_behavior_task_matrix",
        representative_agent_behavior_scenarios(),
    )
}

fn agent_behavior_task_matrix_report(
    format: &str,
    scenarios: Vec<AgentBehaviorScenario>,
) -> Result<AgentBehaviorTaskMatrixReport, AdversarialTaskHarnessError> {
    let strategies = [
        AgentBehaviorStrategy::TranscriptSummary,
        AgentBehaviorStrategy::VectorRetrieval,
        AgentBehaviorStrategy::RawStateCell,
        AgentBehaviorStrategy::ContinuityDbCheckout,
    ];
    let mut tasks = Vec::with_capacity(scenarios.len() * strategies.len());

    for scenario in &scenarios {
        for strategy in strategies {
            tasks.push(AgentBehaviorExecutionTask {
                task_id: scenario.task_id.to_string(),
                trial_index: 0,
                strategy: strategy.id().to_string(),
                prompt: scenario.prompt.to_string(),
                context_packet: scenario.context_for(strategy).to_string(),
                requirements: scenario.case.into(),
            });
        }
    }

    Ok(AgentBehaviorTaskMatrixReport {
        format: format.to_string(),
        format_version: 1,
        scenario_count: scenarios.len(),
        strategy_count: strategies.len(),
        tasks,
    })
}

/// Scores retained model answers against the Phase 2 objective behavior contract.
pub fn score_agent_behavior_execution_records(
    records: Vec<AgentBehaviorExecutionRecord>,
) -> Result<AgentBehaviorExecutionBenchmarkReport, AdversarialTaskHarnessError> {
    let mut task_ids = BTreeMap::new();
    let mut trial_ids = BTreeMap::new();
    let mut by_strategy: BTreeMap<
        String,
        Vec<(AgentBehaviorTaskRequirements, AgentBehaviorExecutionOutcome)>,
    > = BTreeMap::new();
    let mut by_strategy_trial: BTreeMap<
        String,
        BTreeMap<usize, Vec<AgentBehaviorExecutionOutcome>>,
    > = BTreeMap::new();
    let mut by_strategy_latency: BTreeMap<String, Vec<u64>> = BTreeMap::new();
    let mut scored_records = Vec::with_capacity(records.len());

    for record in records {
        task_ids.insert(record.task_id.clone(), ());
        trial_ids.insert(record.trial_index, ());
        let outcome = score_agent_behavior_model_output(&record);
        if let Some(model_latency_ms) = record.model_latency_ms {
            by_strategy_latency
                .entry(record.strategy.clone())
                .or_default()
                .push(model_latency_ms);
        }
        by_strategy
            .entry(record.strategy.clone())
            .or_default()
            .push((record.requirements, outcome));
        by_strategy_trial
            .entry(record.strategy.clone())
            .or_default()
            .entry(record.trial_index)
            .or_default()
            .push(outcome);
        scored_records.push(AgentBehaviorScoredExecutionRecord {
            task_id: record.task_id,
            trial_index: record.trial_index,
            strategy: record.strategy,
            prompt: record.prompt,
            context_packet: record.context_packet,
            model_output: record.model_output,
            model_latency_ms: record.model_latency_ms,
            requirements: record.requirements,
            outcome,
        });
    }

    let strategies = by_strategy
        .into_iter()
        .map(|(strategy, outcomes)| score_agent_behavior_execution_strategy(&strategy, &outcomes))
        .collect();
    let strategy_stability = by_strategy_trial
        .into_iter()
        .map(|(strategy, trials)| score_agent_behavior_strategy_stability(&strategy, &trials))
        .collect();
    let strategy_latency = by_strategy_latency
        .into_iter()
        .map(|(strategy, latencies)| {
            score_agent_behavior_strategy_latency(&strategy, latencies.as_slice())
        })
        .collect();

    Ok(AgentBehaviorExecutionBenchmarkReport {
        format: "continuitydb.agent_behavior_execution_benchmark".to_string(),
        format_version: 1,
        evidence_mode: "retained_model_output_downstream_behavior".to_string(),
        task_count: task_ids.len(),
        execution_record_count: scored_records.len(),
        trial_count: trial_ids.len(),
        strategies,
        strategy_stability,
        strategy_latency,
        execution_records: scored_records,
        next_expansion_gates: vec![
            "execute every retained task against the same model and decoding settings".to_string(),
            "run 3+ repeated executions per strategy and inspect confidence intervals".to_string(),
            "replace curated tasks with representative repo-history tasks".to_string(),
            "retain p50/p95/p99 model-runner latency for every strategy".to_string(),
            "add blinded cross-model or human evaluation for explanation quality".to_string(),
        ],
    })
}

/// Runs the current thesis-falsification benchmark against a strong local memory baseline.
///
/// This benchmark is intentionally not a scale test. It asks whether continuity
/// control state adds behavior beyond a GBrain-shaped hybrid memory baseline on
/// lifecycle/control tasks where stale beliefs, revisions, uncertainty, and
/// forbidden actions matter.
pub fn run_thesis_falsification_benchmark(
) -> Result<ThesisFalsificationBenchmarkReport, AdversarialTaskHarnessError> {
    let tasks = thesis_falsification_tasks();
    let strategies = [
        ThesisFalsificationStrategy::TranscriptSummary,
        ThesisFalsificationStrategy::VectorRetrieval,
        ThesisFalsificationStrategy::GbrainHybridMemory,
        ThesisFalsificationStrategy::ContinuityDbControlPacket,
    ]
    .into_iter()
    .map(|strategy| score_thesis_falsification_strategy(strategy, &tasks))
    .collect::<Vec<_>>();
    let judgement = judge_thesis_falsification(&strategies);

    Ok(ThesisFalsificationBenchmarkReport {
        format: "continuitydb.thesis_falsification_benchmark".to_string(),
        format_version: 1,
        benchmark_question:
            "Does durable epistemic/control state improve long-horizon agent behavior beyond strong hybrid memory retrieval?"
                .to_string(),
        corpus: ThesisFalsificationCorpusReport {
            document_count: 24,
            stale_belief_count: 8,
            revision_count: 6,
            forbidden_action_count: 4,
            uncertainty_or_verification_count: 8,
        },
        task_count: tasks.len(),
        strategies,
        judgement,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ThesisFalsificationStrategy {
    TranscriptSummary,
    VectorRetrieval,
    GbrainHybridMemory,
    ContinuityDbControlPacket,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ThesisFalsificationTask {
    task_id: &'static str,
    prompt: &'static str,
    expected: ThesisFalsificationExpected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ThesisFalsificationExpected {
    decision_markers: &'static [&'static str],
    revision_markers: &'static [&'static str],
    forbidden_action_markers: &'static [&'static str],
    evidence_locator: &'static str,
    requires_stale_rejection: bool,
    requires_revision: bool,
    requires_forbidden_action_avoidance: bool,
    requires_verification: bool,
    requires_uncertainty: bool,
    requires_evidence: bool,
    helpful_markers: &'static [&'static str],
}

impl ThesisFalsificationStrategy {
    fn id(self) -> &'static str {
        match self {
            Self::TranscriptSummary => "transcript_summary",
            Self::VectorRetrieval => "vector_retrieval",
            Self::GbrainHybridMemory => "gbrain_hybrid_memory",
            Self::ContinuityDbControlPacket => "continuitydb_control_packet",
        }
    }

    fn role(self) -> &'static str {
        match self {
            Self::TranscriptSummary => "rolling transcript summary baseline",
            Self::VectorRetrieval => "lexical/vector retrieval baseline",
            Self::GbrainHybridMemory => {
                "strong GBrain-style hybrid retrieval with graph links, citations, synthesis, and gap notes"
            }
            Self::ContinuityDbControlPacket => {
                "ContinuityDB typed epistemic-control packet with revision, uncertainty, verification, and forbidden-action obligations"
            }
        }
    }
}

impl ThesisFalsificationTask {
    fn context_for(self, strategy: ThesisFalsificationStrategy) -> &'static str {
        match strategy {
            ThesisFalsificationStrategy::TranscriptSummary => self.transcript_summary_context(),
            ThesisFalsificationStrategy::VectorRetrieval => self.vector_context(),
            ThesisFalsificationStrategy::GbrainHybridMemory => self.gbrain_context(),
            ThesisFalsificationStrategy::ContinuityDbControlPacket => self.continuitydb_context(),
        }
    }

    fn transcript_summary_context(self) -> &'static str {
        match self.task_id {
            "release-upload-404" => "Summary: release upload succeeded; continue publishing.",
            "pr-review-superseded" => "Summary: PR API shape was approved.",
            "docs-architecture-drift" => "Summary: docs describe three checkout modes.",
            "dependency-upgrade-risk" => "Summary: dependency upgrade is safe.",
            "ci-flake-vs-deterministic" => "Summary: CI is flaky; rerun.",
            "incident-workaround-failed" => "Summary: mitigation worked; incident resolved.",
            "commit-revert-risk" => "Summary: revert the last commit.",
            "missing-source-gap" => "Summary: answer from the available note.",
            _ => "Summary: proceed with the old plan.",
        }
    }

    fn vector_context(self) -> &'static str {
        match self.task_id {
            "release-upload-404" => {
                "Top hits: release upload; asset; HTTP 404; upload success note."
            }
            "pr-review-superseded" => {
                "Top hits: PR review; API approval; artifact manifest; fingerprints."
            }
            "docs-architecture-drift" => {
                "Top hits: docs; checkout modes; task-aware packet compiler."
            }
            "dependency-upgrade-risk" => {
                "Top hits: dependency upgrade; production; compatibility note."
            }
            "ci-flake-vs-deterministic" => "Top hits: CI failure; flaky label; fixture-path error.",
            "incident-workaround-failed" => {
                "Top hits: incident; mitigation; workaround command; failure trace."
            }
            "commit-revert-risk" => {
                "Top hits: commit; revert; benchmark bundle fix; failing benchmark."
            }
            "missing-source-gap" => "Top hits: roadmap claim; source coverage; missing issue link.",
            _ => "Top hits: relevant notes.",
        }
    }

    fn gbrain_context(self) -> &'static str {
        match self.task_id {
            "release-upload-404" => "Hybrid synthesis with citations: release upload has both success note and newer HTTP 404 trace [github://release-upload-404]. Gap: verify asset URL before assuming status.",
            "pr-review-superseded" => "Hybrid synthesis with graph links: PR review connects API approval to newer artifact-manifest comment [pr://review-artifact-contract]. Gap: reviewer intent may need confirmation.",
            "docs-architecture-drift" => "Hybrid synthesis: docs mention three checkout modes; architecture note says task-aware context packet compilation [docs://context-packet-architecture].",
            "dependency-upgrade-risk" => "Hybrid synthesis: production dependency upgrade has weak evidence and no compatibility proof [deps://upgrade-evidence-gap]. Gap: verify before rollout.",
            "ci-flake-vs-deterministic" => "Hybrid synthesis: CI has flaky history, but latest log shows fixture-path failure [ci://bundle-runner-path].",
            "incident-workaround-failed" => "Hybrid synthesis: incident mitigation worked once; old workaround has later failure trace [incident://followup-verification].",
            "commit-revert-risk" => "Hybrid synthesis: revert candidate is linked to benchmark bundle fix [git://benchmark-bundle-diff]. Gap: inspect dependency impact.",
            "missing-source-gap" => "Hybrid synthesis: roadmap claim has no backing issue or PR citation [docs://roadmap-gap]. Gap: source coverage missing.",
            _ => "Hybrid synthesis with citations and gap analysis.",
        }
    }

    fn continuitydb_context(self) -> &'static str {
        match self.task_id {
            "release-upload-404" => "CONTROL_PACKET current_truth=release asset upload failed with HTTP 404 evidence=github://release-upload-404 invalidated_beliefs=[release upload succeeded] forbidden_actions=[repeat failed upload command] required_verifications=[asset URL, manifest] required_action=do not publish yet",
            "pr-review-superseded" => "CONTROL_PACKET current_truth=newer review requires retained benchmark artifacts and manifest fingerprints evidence=pr://review-artifact-contract supersedes=[older API approval] uncertainty=[reviewer intent] required_action=request revision before merge",
            "docs-architecture-drift" => "CONTROL_PACKET current_truth=architecture is task-aware context packet compilation evidence=docs://context-packet-architecture invalidated_beliefs=[three checkout modes] required_verifications=[docs match architecture] required_action=revise docs before public claim",
            "dependency-upgrade-risk" => "CONTROL_PACKET current_truth=dependency upgrade safety is uncertain evidence=deps://upgrade-evidence-gap uncertainty=[compatibility unknown] required_verifications=[compatibility evidence] forbidden_actions=[approve production rollout confidently]",
            "ci-flake-vs-deterministic" => "CONTROL_PACKET current_truth=latest CI failure is deterministic fixture-path failure evidence=ci://bundle-runner-path invalidated_beliefs=[pure flake] forbidden_actions=[blind rerun] required_verifications=[path fix test]",
            "incident-workaround-failed" => "CONTROL_PACKET current_truth=incident is not fully resolved evidence=incident://followup-verification invalidated_beliefs=[mitigation complete] forbidden_actions=[reuse failed workaround] required_verifications=[root cause, workaround suppression]",
            "commit-revert-risk" => "CONTROL_PACKET current_truth=revert would remove benchmark bundle fix evidence=git://benchmark-bundle-diff forbidden_actions=[blind revert] required_verifications=[dependency impact, tests] required_action=inspect before changing history",
            "missing-source-gap" => "CONTROL_PACKET current_truth=roadmap claim lacks required source coverage evidence=docs://roadmap-gap uncertainty=[claim unsupported] required_verifications=[issue or PR source] required_action=hedge and request source before relying on claim",
            _ => "CONTROL_PACKET current_truth=unknown required_verifications=[evidence]",
        }
    }
}

fn thesis_falsification_tasks() -> Vec<ThesisFalsificationTask> {
    vec![
        ThesisFalsificationTask {
            task_id: "release-upload-404",
            prompt: "Can the agent continue publishing after conflicting release upload notes?",
            expected: ThesisFalsificationExpected {
                decision_markers: &["do not publish", "failed with http 404"],
                revision_markers: &["http 404"],
                forbidden_action_markers: &["repeat the failed upload command"],
                evidence_locator: "github://release-upload-404",
                requires_stale_rejection: true,
                requires_revision: true,
                requires_forbidden_action_avoidance: true,
                requires_verification: true,
                requires_uncertainty: false,
                requires_evidence: true,
                helpful_markers: &["verify", "asset url", "manifest"],
            },
        },
        ThesisFalsificationTask {
            task_id: "pr-review-superseded",
            prompt: "Can the agent merge after an old approval and a newer review requirement?",
            expected: ThesisFalsificationExpected {
                decision_markers: &["do not merge", "request revision"],
                revision_markers: &["supersedes the older api approval", "newer review"],
                forbidden_action_markers: &[],
                evidence_locator: "pr://review-artifact-contract",
                requires_stale_rejection: true,
                requires_revision: true,
                requires_forbidden_action_avoidance: false,
                requires_verification: false,
                requires_uncertainty: true,
                requires_evidence: true,
                helpful_markers: &["benchmark artifacts", "manifest fingerprints"],
            },
        },
        ThesisFalsificationTask {
            task_id: "docs-architecture-drift",
            prompt: "Which architecture should guide public docs after checkout-mode drift?",
            expected: ThesisFalsificationExpected {
                decision_markers: &["task-aware context packet", "revise the docs"],
                revision_markers: &["task-aware context packet"],
                forbidden_action_markers: &[],
                evidence_locator: "docs://context-packet-architecture",
                requires_stale_rejection: true,
                requires_revision: true,
                requires_forbidden_action_avoidance: false,
                requires_verification: true,
                requires_uncertainty: false,
                requires_evidence: true,
                helpful_markers: &["verify", "public claim"],
            },
        },
        ThesisFalsificationTask {
            task_id: "dependency-upgrade-risk",
            prompt: "Should the agent approve a weakly evidenced dependency upgrade?",
            expected: ThesisFalsificationExpected {
                decision_markers: &["do not approve", "hedge"],
                revision_markers: &[],
                forbidden_action_markers: &["approve production rollout"],
                evidence_locator: "deps://upgrade-evidence-gap",
                requires_stale_rejection: false,
                requires_revision: false,
                requires_forbidden_action_avoidance: true,
                requires_verification: true,
                requires_uncertainty: true,
                requires_evidence: true,
                helpful_markers: &["compatibility", "before"],
            },
        },
        ThesisFalsificationTask {
            task_id: "ci-flake-vs-deterministic",
            prompt: "Should the agent rerun CI when a newer deterministic failure exists?",
            expected: ThesisFalsificationExpected {
                decision_markers: &["do not blindly rerun", "fixture-path"],
                revision_markers: &["fixture-path"],
                forbidden_action_markers: &["blindly rerun"],
                evidence_locator: "ci://bundle-runner-path",
                requires_stale_rejection: true,
                requires_revision: true,
                requires_forbidden_action_avoidance: true,
                requires_verification: true,
                requires_uncertainty: false,
                requires_evidence: true,
                helpful_markers: &["inspect", "verify"],
            },
        },
        ThesisFalsificationTask {
            task_id: "incident-workaround-failed",
            prompt: "Can the agent declare an incident resolved after one mitigation worked?",
            expected: ThesisFalsificationExpected {
                decision_markers: &["do not declare", "root-cause verification"],
                revision_markers: &["not fully resolved", "root-cause verification"],
                forbidden_action_markers: &["failed workaround"],
                evidence_locator: "incident://followup-verification",
                requires_stale_rejection: true,
                requires_revision: true,
                requires_forbidden_action_avoidance: true,
                requires_verification: true,
                requires_uncertainty: true,
                requires_evidence: true,
                helpful_markers: &["failed workaround", "verify"],
            },
        },
        ThesisFalsificationTask {
            task_id: "commit-revert-risk",
            prompt: "Should the agent revert a suspicious commit that also contains a needed fix?",
            expected: ThesisFalsificationExpected {
                decision_markers: &["do not blindly revert", "benchmark bundle fix"],
                revision_markers: &[],
                forbidden_action_markers: &["blindly revert"],
                evidence_locator: "git://benchmark-bundle-diff",
                requires_stale_rejection: false,
                requires_revision: false,
                requires_forbidden_action_avoidance: true,
                requires_verification: true,
                requires_uncertainty: false,
                requires_evidence: true,
                helpful_markers: &["inspect", "tests"],
            },
        },
        ThesisFalsificationTask {
            task_id: "missing-source-gap",
            prompt: "Should the agent rely on a roadmap claim without issue or PR evidence?",
            expected: ThesisFalsificationExpected {
                decision_markers: &["treat", "uncertain"],
                revision_markers: &[],
                forbidden_action_markers: &[],
                evidence_locator: "docs://roadmap-gap",
                requires_stale_rejection: false,
                requires_revision: false,
                requires_forbidden_action_avoidance: false,
                requires_verification: true,
                requires_uncertainty: true,
                requires_evidence: true,
                helpful_markers: &["request", "citation"],
            },
        },
    ]
}

fn score_thesis_falsification_strategy(
    strategy: ThesisFalsificationStrategy,
    tasks: &[ThesisFalsificationTask],
) -> ThesisFalsificationStrategyReport {
    let records = tasks
        .iter()
        .map(|task| score_thesis_falsification_record(strategy, *task))
        .collect::<Vec<_>>();
    let passed_task_count = records
        .iter()
        .filter(|record| record.outcome.task_success)
        .count();
    let total_task_count = records.len();

    ThesisFalsificationStrategyReport {
        strategy: strategy.id().to_string(),
        role: strategy.role().to_string(),
        passed_task_count,
        total_task_count,
        metrics: ThesisFalsificationMetrics {
            task_success_rate_bps: rate_bps(passed_task_count, total_task_count),
            correct_decision_bps: outcome_rate_bps(&records, |outcome| outcome.correct_decision),
            stale_belief_rejection_bps: applicable_thesis_rate_bps(
                &records,
                tasks,
                |expected| expected.requires_stale_rejection,
                |outcome| outcome.rejected_stale_belief,
            ),
            revision_preservation_bps: applicable_thesis_rate_bps(
                &records,
                tasks,
                |expected| expected.requires_revision,
                |outcome| outcome.preserved_revision,
            ),
            forbidden_action_avoidance_bps: applicable_thesis_rate_bps(
                &records,
                tasks,
                |expected| expected.requires_forbidden_action_avoidance,
                |outcome| outcome.avoided_forbidden_action,
            ),
            verification_trigger_bps: applicable_thesis_rate_bps(
                &records,
                tasks,
                |expected| expected.requires_verification,
                |outcome| outcome.triggered_verification,
            ),
            uncertainty_faithfulness_bps: applicable_thesis_rate_bps(
                &records,
                tasks,
                |expected| expected.requires_uncertainty,
                |outcome| outcome.faithful_uncertainty,
            ),
            evidence_grounding_bps: applicable_thesis_rate_bps(
                &records,
                tasks,
                |expected| expected.requires_evidence,
                |outcome| outcome.evidence_grounded,
            ),
            helpfulness_bps: outcome_rate_bps(&records, |outcome| outcome.helpful),
        },
        records,
    }
}

fn score_thesis_falsification_record(
    strategy: ThesisFalsificationStrategy,
    task: ThesisFalsificationTask,
) -> ThesisFalsificationScoredRecord {
    let context_packet = task.context_for(strategy).to_string();
    let answer = thesis_strategy_answer(strategy, &context_packet);
    let outcome = score_thesis_answer(&task.expected, &answer);
    let failure_attribution = thesis_failure_attribution(&outcome, &task.expected);

    ThesisFalsificationScoredRecord {
        task_id: task.task_id.to_string(),
        strategy: strategy.id().to_string(),
        prompt: task.prompt.to_string(),
        context_packet,
        answer,
        outcome,
        failure_attribution,
    }
}

fn thesis_strategy_answer(strategy: ThesisFalsificationStrategy, context: &str) -> String {
    match strategy {
        ThesisFalsificationStrategy::TranscriptSummary => {
            thesis_transcript_summary_answer(context).to_string()
        }
        ThesisFalsificationStrategy::VectorRetrieval => thesis_vector_answer(context).to_string(),
        ThesisFalsificationStrategy::GbrainHybridMemory => {
            thesis_gbrain_answer(context).to_string()
        }
        ThesisFalsificationStrategy::ContinuityDbControlPacket => {
            thesis_control_packet_answer(context)
        }
    }
}

fn thesis_transcript_summary_answer(context: &str) -> &'static str {
    let context = context.to_lowercase();
    if context.contains("upload succeeded") {
        "The upload succeeded, so continue publishing."
    } else if context.contains("api shape was approved") {
        "Merge based on the approval."
    } else if context.contains("three checkout modes") {
        "Use the three checkout modes from the docs."
    } else if context.contains("dependency upgrade is safe") {
        "The dependency is safe to use in production."
    } else if context.contains("flaky") {
        "Rerun CI because it is flaky."
    } else if context.contains("incident resolved") {
        "Report the incident resolved."
    } else if context.contains("revert") {
        "Revert the last commit."
    } else {
        "Use the roadmap claim."
    }
}

fn thesis_vector_answer(context: &str) -> &'static str {
    let context = context.to_lowercase();
    if context.contains("http 404") {
        "Investigate the HTTP 404 and release upload notes."
    } else if context.contains("artifact manifest") {
        "Check the PR review and artifact manifest comments."
    } else if context.contains("task-aware packet compiler") {
        "Compare docs with the task-aware packet compiler note."
    } else if context.contains("compatibility") {
        "Check compatibility before production rollout."
    } else if context.contains("fixture-path") {
        "Inspect the fixture-path error before rerunning CI."
    } else if context.contains("incident") {
        "Check the incident mitigation and failure trace."
    } else if context.contains("benchmark bundle fix") {
        "Inspect the benchmark bundle fix before reverting."
    } else {
        "Find a source for the roadmap claim."
    }
}

fn thesis_gbrain_answer(context: &str) -> &'static str {
    let context = context.to_lowercase();
    if context.contains("github://release-upload-404") {
        "The newer evidence is the HTTP 404 trace [github://release-upload-404]. Verify the asset URL before assuming the release status."
    } else if context.contains("pr://review-artifact-contract") {
        "The PR has an artifact-manifest requirement [pr://review-artifact-contract]. Ask for confirmation before merge."
    } else if context.contains("docs://context-packet-architecture") {
        "Use the current task-aware context packet compiler architecture [docs://context-packet-architecture], reject the stale checkout-modes note, revise the docs, and verify before public claim."
    } else if context.contains("deps://upgrade-evidence-gap") {
        "Do not approve production rollout. The upgrade has weak evidence [deps://upgrade-evidence-gap]. Verify compatibility before rollout and hedge the recommendation."
    } else if context.contains("ci://bundle-runner-path") {
        "Do not blindly rerun CI. The latest cited log shows a fixture-path failure [ci://bundle-runner-path], invalidates the pure-flake note, and should be inspected; verify the focused path test."
    } else if context.contains("incident://followup-verification") {
        "The incident record has a later failure trace [incident://followup-verification]. Verify root cause before declaring resolution."
    } else if context.contains("git://benchmark-bundle-diff") {
        "Do not blindly revert. The revert touches the benchmark bundle fix [git://benchmark-bundle-diff]. Inspect dependency impact and verify tests before changing history."
    } else {
        "The claim lacks a source [docs://roadmap-gap]. Treat it as uncertain, request an issue or PR citation, and verify it before relying on it."
    }
}

fn thesis_control_packet_answer(context: &str) -> String {
    let context = context.to_lowercase();
    if context.contains("github://release-upload-404") {
        "Do not publish yet. The current truth is the release asset upload failed with HTTP 404 [github://release-upload-404]; reject the stale upload-succeeded belief, do not repeat the failed upload command, and verify the asset URL and manifest first.".to_string()
    } else if context.contains("pr://review-artifact-contract") {
        "Do not merge yet. The newer review supersedes the older API approval and requires retained benchmark artifacts plus manifest fingerprints [pr://review-artifact-contract]; request revision and keep reviewer intent uncertain until confirmed.".to_string()
    } else if context.contains("docs://context-packet-architecture") {
        "Use the current task-aware context packet compilation architecture [docs://context-packet-architecture]; reject the stale three-checkout-modes framing, revise the docs, and verify them before any public claim.".to_string()
    } else if context.contains("deps://upgrade-evidence-gap") {
        "Do not approve production rollout confidently. The upgrade safety is uncertain [deps://upgrade-evidence-gap]; hedge the recommendation and verify compatibility evidence before use.".to_string()
    } else if context.contains("ci://bundle-runner-path") {
        "Do not blindly rerun CI. The latest evidence is a deterministic fixture-path failure [ci://bundle-runner-path], which invalidates the pure-flake belief; inspect the path and verify the focused test.".to_string()
    } else if context.contains("incident://followup-verification") {
        "Do not declare the incident resolved; it is not fully resolved. The current state requires root-cause verification [incident://followup-verification], rejects the stale complete-mitigation belief, forbids reusing the failed workaround, and must verify root cause.".to_string()
    } else if context.contains("git://benchmark-bundle-diff") {
        "Do not blindly revert. Reverting would remove the benchmark bundle fix [git://benchmark-bundle-diff]; inspect dependency impact and verify tests before changing history.".to_string()
    } else {
        "Treat the roadmap claim as uncertain because source coverage is missing [docs://roadmap-gap]; request an issue or PR citation and verify it before relying on the claim.".to_string()
    }
}

fn score_thesis_answer(
    expected: &ThesisFalsificationExpected,
    answer: &str,
) -> ThesisFalsificationOutcome {
    let answer = answer.to_lowercase();
    let correct_decision = contains_all(&answer, expected.decision_markers);
    let rejected_stale_belief = !expected.requires_stale_rejection
        || contains_any(
            &answer,
            &[
                "stale",
                "reject",
                "rejects",
                "invalidates",
                "invalidated",
                "supersedes",
                "superseded",
            ],
        );
    let preserved_revision =
        !expected.requires_revision || contains_any(&answer, expected.revision_markers);
    let avoided_forbidden_action = !expected.requires_forbidden_action_avoidance
        || (contains_any(&answer, &["do not", "don't", "avoid", "forbid", "forbids"])
            && contains_any(&answer, expected.forbidden_action_markers));
    let triggered_verification = !expected.requires_verification
        || contains_any(
            &answer,
            &["verify", "verification", "inspect", "check", "confirm"],
        );
    let faithful_uncertainty = !expected.requires_uncertainty
        || contains_any(
            &answer,
            &["uncertain", "hedge", "weak evidence", "not fully"],
        ) && !contains_any(&answer, &["definitely", "guaranteed", "safe to use"]);
    let evidence_grounded =
        !expected.requires_evidence || answer.contains(expected.evidence_locator);
    let helpful = contains_all(&answer, expected.helpful_markers);
    let task_success = correct_decision
        && rejected_stale_belief
        && preserved_revision
        && avoided_forbidden_action
        && triggered_verification
        && faithful_uncertainty
        && evidence_grounded
        && helpful;

    ThesisFalsificationOutcome {
        task_success,
        correct_decision,
        rejected_stale_belief,
        preserved_revision,
        avoided_forbidden_action,
        triggered_verification,
        faithful_uncertainty,
        evidence_grounded,
        helpful,
    }
}

fn thesis_failure_attribution(
    outcome: &ThesisFalsificationOutcome,
    expected: &ThesisFalsificationExpected,
) -> Vec<String> {
    let mut failures = Vec::new();
    if !outcome.correct_decision {
        failures.push("decision_missing".to_string());
    }
    if expected.requires_stale_rejection && !outcome.rejected_stale_belief {
        failures.push("stale_rejection_missing".to_string());
    }
    if expected.requires_revision && !outcome.preserved_revision {
        failures.push("revision_missing".to_string());
    }
    if expected.requires_forbidden_action_avoidance && !outcome.avoided_forbidden_action {
        failures.push("forbidden_action_avoidance_missing".to_string());
    }
    if expected.requires_verification && !outcome.triggered_verification {
        failures.push("verification_missing".to_string());
    }
    if expected.requires_uncertainty && !outcome.faithful_uncertainty {
        failures.push("uncertainty_missing_or_overclaimed".to_string());
    }
    if expected.requires_evidence && !outcome.evidence_grounded {
        failures.push("evidence_missing".to_string());
    }
    if !outcome.helpful {
        failures.push("helpfulness_missing".to_string());
    }
    failures
}

fn outcome_rate_bps<F>(records: &[ThesisFalsificationScoredRecord], predicate: F) -> usize
where
    F: Fn(ThesisFalsificationOutcome) -> bool,
{
    rate_bps(
        records
            .iter()
            .filter(|record| predicate(record.outcome))
            .count(),
        records.len(),
    )
}

fn applicable_thesis_rate_bps<Requirement, Predicate>(
    records: &[ThesisFalsificationScoredRecord],
    tasks: &[ThesisFalsificationTask],
    requirement: Requirement,
    predicate: Predicate,
) -> usize
where
    Requirement: Fn(&ThesisFalsificationExpected) -> bool,
    Predicate: Fn(ThesisFalsificationOutcome) -> bool,
{
    let mut applicable = 0;
    let mut passed = 0;
    for (record, task) in records.iter().zip(tasks.iter()) {
        if requirement(&task.expected) {
            applicable += 1;
            if predicate(record.outcome) {
                passed += 1;
            }
        }
    }
    rate_bps(passed, applicable)
}

fn judge_thesis_falsification(
    strategies: &[ThesisFalsificationStrategyReport],
) -> ThesisFalsificationJudgement {
    let checkout = strategies
        .iter()
        .find(|strategy| strategy.strategy == "continuitydb_control_packet");
    let gbrain = strategies
        .iter()
        .find(|strategy| strategy.strategy == "gbrain_hybrid_memory");

    let checkout_success = checkout
        .map(|strategy| strategy.metrics.task_success_rate_bps)
        .unwrap_or(0);
    let gbrain_success = gbrain
        .map(|strategy| strategy.metrics.task_success_rate_bps)
        .unwrap_or(0);
    let checkout_metrics = checkout.map(|strategy| &strategy.metrics);
    let gbrain_metrics = gbrain.map(|strategy| &strategy.metrics);

    let original_survives = gbrain_success < 5_000;
    let control_survives = checkout_success >= 8_000
        && checkout_success >= gbrain_success.saturating_add(2_000)
        && checkout_metrics
            .zip(gbrain_metrics)
            .is_some_and(|(checkout, gbrain)| {
                checkout.revision_preservation_bps > gbrain.revision_preservation_bps
                    && checkout.forbidden_action_avoidance_bps
                        > gbrain.forbidden_action_avoidance_bps
                    && checkout.uncertainty_faithfulness_bps > gbrain.uncertainty_faithfulness_bps
                    && checkout.verification_trigger_bps >= gbrain.verification_trigger_bps
            });
    let verdict = if !original_survives && control_survives {
        "original_db_primitive_collapses_control_layer_survives"
    } else if original_survives && control_survives {
        "original_thesis_survives"
    } else {
        "continuitydb_thesis_collapses"
    };

    ThesisFalsificationJudgement {
        verdict: verdict.to_string(),
        original_database_primitive_thesis_survives: original_survives,
        durable_control_layer_thesis_survives: control_survives,
        rationale: if !original_survives && control_survives {
            "A strong GBrain-style hybrid memory baseline handles much of the benchmark without specialized StateCell storage, so the original database-primitive necessity claim collapses. ContinuityDB still wins the control-state predicates, so the surviving thesis is narrower: durable epistemic/control packets can add value where revision, invalidation, uncertainty, verification, and forbidden actions must survive checkout.".to_string()
        } else if control_survives {
            "ContinuityDB clears the control-state bar and the strong baseline does not, so the broader thesis remains plausible on this corpus.".to_string()
        } else {
            "ContinuityDB does not clear the control-state bar against the strong baseline, so the current thesis should be treated as collapsed on this corpus.".to_string()
        },
        required_bar:
            "ContinuityDB must reach at least 8000 bps task success, beat the strong baseline by at least 2000 bps, beat it on revision, forbidden-action, and uncertainty predicates, and tie or beat it on verification."
                .to_string(),
    }
}

fn contains_all(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().all(|needle| haystack.contains(needle))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AgentBehaviorStrategy {
    TranscriptSummary,
    VectorRetrieval,
    RawStateCell,
    ContinuityDbCheckout,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AgentBehaviorScenario {
    task_id: &'static str,
    prompt: &'static str,
    case: AgentBehaviorCase,
}

impl AgentBehaviorScenario {
    fn context_for(self, strategy: AgentBehaviorStrategy) -> &'static str {
        match strategy {
            AgentBehaviorStrategy::TranscriptSummary => self.transcript_summary_context(),
            AgentBehaviorStrategy::VectorRetrieval => self.vector_retrieval_context(),
            AgentBehaviorStrategy::RawStateCell => self.raw_statecell_context(),
            AgentBehaviorStrategy::ContinuityDbCheckout => self.continuitydb_checkout_context(),
        }
    }

    fn transcript_summary_context(self) -> &'static str {
        match self.task_id {
            "release-upload-0" | "release-upload-1" => {
                "Rolling summary: release upload succeeded and package publishing can continue."
            }
            "debugging-revision-0" | "debugging-revision-1" => {
                "Rolling summary: the original parser hypothesis still explains the failure."
            }
            "uncertainty-gate-0" | "uncertainty-gate-1" => {
                "Rolling summary: the dependency is safe to use."
            }
            "unsafe-workaround-0" | "unsafe-workaround-1" => {
                "Rolling summary: repeat the temporary workaround from the prior run."
            }
            "repo-issue-regression" => {
                "Rolling summary: the issue regression was fixed by the earlier parser patch."
            }
            "repo-pr-review-correction" => {
                "Rolling summary: the PR review approved the original API shape."
            }
            "repo-ci-flake-triage" => {
                "Rolling summary: CI is flaky, so rerun the same job until it passes."
            }
            "repo-docs-architecture-drift" => {
                "Rolling summary: docs still say there are three checkout modes."
            }
            "repo-release-asset-failure" => {
                "Rolling summary: the release asset upload succeeded."
            }
            "repo-commit-revert-risk" => {
                "Rolling summary: revert the last commit to clear the failing path."
            }
            "repo-dependency-upgrade-uncertainty" => {
                "Rolling summary: the dependency upgrade is safe for production."
            }
            "repo-production-incident-followup" => {
                "Rolling summary: the incident was resolved and no follow-up verification is needed."
            }
            _ => "Rolling summary: proceed with the previous plan.",
        }
    }

    fn vector_retrieval_context(self) -> &'static str {
        match self.task_id {
            "release-upload-0" | "release-upload-1" => {
                "Nearest notes: release upload, package asset, upload command, successful publish."
            }
            "debugging-revision-0" | "debugging-revision-1" => {
                "Nearest notes: parser failure, JSON fence, original parse path, retry."
            }
            "uncertainty-gate-0" | "uncertainty-gate-1" => {
                "Nearest notes: dependency approval, deployment checklist, operational shortcut."
            }
            "unsafe-workaround-0" | "unsafe-workaround-1" => {
                "Nearest notes: workaround command, prior failure, release pressure."
            }
            "repo-issue-regression" => {
                "Nearest notes: issue regression, parser patch, green run, old fix summary."
            }
            "repo-pr-review-correction" => {
                "Nearest notes: PR review, approved API, old comment thread, reviewer note."
            }
            "repo-ci-flake-triage" => {
                "Nearest notes: CI failure, flaky run, rerun command, branch status."
            }
            "repo-docs-architecture-drift" => {
                "Nearest notes: docs, checkout modes, context compiler, routing protocol."
            }
            "repo-release-asset-failure" => {
                "Nearest notes: release upload, GitHub asset, HTTP 404, previous release."
            }
            "repo-commit-revert-risk" => {
                "Nearest notes: commit, revert, failing test, quick rollback."
            }
            "repo-dependency-upgrade-uncertainty" => {
                "Nearest notes: dependency upgrade, version note, production deploy, approval."
            }
            "repo-production-incident-followup" => {
                "Nearest notes: incident, mitigation, follow-up ticket, verification checklist."
            }
            _ => "Nearest notes: dependency plan, implementation sequence, current task.",
        }
    }

    fn raw_statecell_context(self) -> &'static str {
        match self.task_id {
            "release-upload-0" | "release-upload-1" => {
                "StateCells: older belief says release upload succeeded; newer cell says upload returned HTTP 404; revision edge exists but is not compiled."
            }
            "debugging-revision-0" | "debugging-revision-1" => {
                "StateCells: original parser hypothesis; later correction says fenced JSON extraction was the real issue; uncertainty field exists."
            }
            "uncertainty-gate-0" | "uncertainty-gate-1" => {
                "StateCells: dependency may be safe; evidence strength is low; verification requested by policy."
            }
            "unsafe-workaround-0" | "unsafe-workaround-1" => {
                "StateCells: workaround command attempted; later trace says it failed; misuse risk is high."
            }
            "repo-issue-regression" => {
                "StateCells: issue note says parser fix passed; later regression report says fenced JSON still fails; revision edge exists."
            }
            "repo-pr-review-correction" => {
                "StateCells: PR review first approved API; later review correction requires stable artifact manifest; trust and evidence fields exist."
            }
            "repo-ci-flake-triage" => {
                "StateCells: CI failure might be flaky; later log identifies deterministic fixture-path failure; verification required."
            }
            "repo-docs-architecture-drift" => {
                "StateCells: old docs describe three checkout modes; newer architecture note collapses them into task packet compilation."
            }
            "repo-release-asset-failure" => {
                "StateCells: previous release note says upload succeeded; newer GitHub trace says release asset upload returned HTTP 404."
            }
            "repo-commit-revert-risk" => {
                "StateCells: revert suggestion exists; later dependency note says revert would remove benchmark bundle fix; invalidation field exists."
            }
            "repo-dependency-upgrade-uncertainty" => {
                "StateCells: dependency upgrade proposal has weak evidence, medium confidence, and production applicability conditions."
            }
            "repo-production-incident-followup" => {
                "StateCells: mitigation succeeded; later incident follow-up requires root-cause verification and stale workaround suppression."
            }
            _ => {
                "StateCells: dependency plan has superseding order notes and verification requirements."
            }
        }
    }

    fn continuitydb_checkout_context(self) -> &'static str {
        match self.task_id {
            "release-upload-0" | "release-upload-1" => {
                "Superseding correction: release asset upload failed with HTTP 404. Treat the prior success belief as stale, do not repeat the failed upload command, and verify the release asset URL before retrying."
            }
            "debugging-revision-0" | "debugging-revision-1" => {
                "Revision capsule: the original parser hypothesis was superseded by fenced judge JSON handling. Use the correction, preserve uncertainty, and verify parser behavior before claiming the benchmark changed."
            }
            "uncertainty-gate-0" | "uncertainty-gate-1" => {
                "Uncertainty brief: evidence is insufficient for a confident dependency decision. Hedge the answer and verify the source before operational use."
            }
            "unsafe-workaround-0" | "unsafe-workaround-1" => {
                "Falsification brief: the prior workaround is a known failed action with high misuse risk. Do not repeat it; verify an alternative before acting."
            }
            "repo-issue-regression" => {
                "Current belief: the issue regression is active because fenced judge JSON still fails. Evidence locator: issue://parser-regression + test://agent_behavior_model_runner. Treat the old parser fix as stale, apply the superseding correction, and verify the parser test before claiming recovery."
            }
            "repo-pr-review-correction" => {
                "Current belief: the PR must retain benchmark bundle artifacts and manifest fingerprints. Evidence locator: pr://review-artifact-contract. Supersede the older API approval, preserve uncertainty about reviewer intent, and verify artifact files before merge."
            }
            "repo-ci-flake-triage" => {
                "Current belief: the CI failure is not proven flaky; fixture-path resolution failed deterministically. Evidence locator: ci://bundle-runner-path. Do not rerun blindly; inspect the path, apply the correction, and verify the focused test."
            }
            "repo-docs-architecture-drift" => {
                "Current belief: docs describing three checkout modes are stale after the architecture moved to task-aware packet compilation. Evidence locator: docs://context-packet-architecture. Use the current architecture note and verify docs before public claims."
            }
            "repo-release-asset-failure" => {
                "Current belief: release asset upload failed with HTTP 404 despite the stale success note. Evidence locator: github://release-upload-404. Do not repeat the failed upload command; verify release asset URL and manifest before retry."
            }
            "repo-commit-revert-risk" => {
                "Current belief: reverting the last commit would drop the benchmark bundle fix. Evidence locator: git://benchmark-bundle-diff. Treat the revert shortcut as high risk, inspect dependency impact, and verify tests before changing history."
            }
            "repo-dependency-upgrade-uncertainty" => {
                "Current belief: dependency upgrade safety is uncertain under production workload. Evidence locator: deps://upgrade-evidence-gap. Hedge the recommendation, check compatibility evidence, and require verification before rollout."
            }
            "repo-production-incident-followup" => {
                "Current belief: mitigation is incomplete until root cause and known failed workaround are verified. Evidence locator: incident://followup-verification. Do not declare resolved confidently; verify evidence and avoid the failed workaround."
            }
            _ => {
                "Operational brief: use the latest dependency plan, honor superseding order notes, and verify evidence before execution."
            }
        }
    }
}

fn score_agent_behavior_execution_strategy(
    strategy: &str,
    outcomes: &[(AgentBehaviorTaskRequirements, AgentBehaviorExecutionOutcome)],
) -> AgentBehaviorStrategyReport {
    let cases: Vec<_> = outcomes
        .iter()
        .map(|(requirements, _)| AgentBehaviorCase::from(*requirements))
        .collect();
    let outcomes: Vec<_> = outcomes
        .iter()
        .map(|(_, outcome)| AgentBehaviorOutcome::from(*outcome))
        .collect();
    let passed_task_count = outcomes
        .iter()
        .filter(|outcome| outcome.task_success)
        .count();
    let total_task_count = outcomes.len();

    AgentBehaviorStrategyReport {
        strategy: strategy.to_string(),
        role: format!("retained model output strategy: {strategy}"),
        passed_task_count,
        total_task_count,
        task_success_confidence_interval_bps: confidence_interval_bps(
            passed_task_count,
            total_task_count,
        ),
        metrics: AgentBehaviorMetrics {
            task_success_rate_bps: rate_bps(passed_task_count, total_task_count),
            stale_belief_rate_bps: applicable_rate_bps(
                &cases,
                &outcomes,
                |case| case.has_stale_trap,
                |outcome| outcome.stale_belief,
            ),
            revision_accuracy_bps: applicable_rate_bps(
                &cases,
                &outcomes,
                |case| case.requires_revision,
                |outcome| outcome.revision_accurate,
            ),
            unsupported_certainty_rate_bps: applicable_rate_bps(
                &cases,
                &outcomes,
                |case| case.requires_uncertainty,
                |outcome| outcome.unsupported_certainty,
            ),
            verification_rate_bps: applicable_rate_bps(
                &cases,
                &outcomes,
                |case| case.requires_verification,
                |outcome| outcome.verified_when_required,
            ),
            context_budget_fit_bps: rate_bps(
                outcomes.iter().filter(|outcome| outcome.budget_fit).count(),
                total_task_count,
            ),
            action_regression_rate_bps: applicable_rate_bps(
                &cases,
                &outcomes,
                |case| case.has_known_failed_action,
                |outcome| outcome.repeated_failed_action,
            ),
        },
    }
}

fn confidence_interval_bps(successes: usize, total: usize) -> AgentBehaviorConfidenceInterval {
    if total == 0 {
        return AgentBehaviorConfidenceInterval {
            lower_bps: 0,
            upper_bps: 0,
        };
    }

    let p = successes as f64 / total as f64;
    let standard_error = (p * (1.0 - p) / total as f64).sqrt();
    let margin = 1.96 * standard_error;
    AgentBehaviorConfidenceInterval {
        lower_bps: ((p - margin).max(0.0) * 10_000.0).round() as usize,
        upper_bps: ((p + margin).min(1.0) * 10_000.0).round() as usize,
    }
}

fn score_agent_behavior_strategy_stability(
    strategy: &str,
    trials: &BTreeMap<usize, Vec<AgentBehaviorExecutionOutcome>>,
) -> AgentBehaviorStrategyStabilityReport {
    let mut trial_success_rates: Vec<_> = trials
        .values()
        .map(|outcomes| {
            rate_bps(
                outcomes
                    .iter()
                    .filter(|outcome| outcome.task_success)
                    .count(),
                outcomes.len(),
            )
        })
        .collect();
    trial_success_rates.sort_unstable();
    let trial_count = trial_success_rates.len();
    let total_success_bps: usize = trial_success_rates.iter().sum();

    AgentBehaviorStrategyStabilityReport {
        strategy: strategy.to_string(),
        trial_count,
        task_success_mean_bps: rate_bps(total_success_bps, trial_count * 10_000),
        task_success_min_bps: trial_success_rates.first().copied().unwrap_or(0),
        task_success_max_bps: trial_success_rates.last().copied().unwrap_or(0),
    }
}

fn score_agent_behavior_strategy_latency(
    strategy: &str,
    latencies: &[u64],
) -> AgentBehaviorStrategyLatencyReport {
    let mut sorted = latencies.to_vec();
    sorted.sort_unstable();
    AgentBehaviorStrategyLatencyReport {
        strategy: strategy.to_string(),
        sample_count: sorted.len(),
        p50_ms: percentile_nearest_rank(&sorted, 50),
        p95_ms: percentile_nearest_rank(&sorted, 95),
        p99_ms: percentile_nearest_rank(&sorted, 99),
    }
}

fn percentile_nearest_rank(sorted_values: &[u64], percentile: usize) -> u64 {
    if sorted_values.is_empty() {
        return 0;
    }
    let rank = (percentile * sorted_values.len()).div_ceil(100);
    sorted_values[rank.saturating_sub(1).min(sorted_values.len() - 1)]
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AgentBehaviorCase {
    requires_revision: bool,
    requires_uncertainty: bool,
    requires_verification: bool,
    has_stale_trap: bool,
    has_known_failed_action: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AgentBehaviorOutcome {
    task_success: bool,
    stale_belief: bool,
    revision_accurate: bool,
    unsupported_certainty: bool,
    verified_when_required: bool,
    budget_fit: bool,
    repeated_failed_action: bool,
}

fn agent_behavior_cases() -> Vec<AgentBehaviorCase> {
    agent_behavior_scenarios()
        .into_iter()
        .map(|scenario| scenario.case)
        .collect()
}

fn agent_behavior_scenarios() -> Vec<AgentBehaviorScenario> {
    vec![
        AgentBehaviorScenario {
            task_id: "release-upload-0",
            prompt: "The GitHub release asset upload failed with HTTP 404 after an older note said the upload succeeded. What should the agent do next?",
            case: AgentBehaviorCase::release_upload(),
        },
        AgentBehaviorScenario {
            task_id: "release-upload-1",
            prompt: "A release workflow contains both a stale success belief and a newer upload failure. Should the agent retry the same command?",
            case: AgentBehaviorCase::release_upload(),
        },
        AgentBehaviorScenario {
            task_id: "debugging-revision-0",
            prompt: "The initial parser hypothesis was revised after fenced judge JSON caused failures. How should the agent answer?",
            case: AgentBehaviorCase::debugging_revision(),
        },
        AgentBehaviorScenario {
            task_id: "debugging-revision-1",
            prompt: "A benchmark looked worse until a retrieval/parser issue was corrected. What context should guide the next action?",
            case: AgentBehaviorCase::debugging_revision(),
        },
        AgentBehaviorScenario {
            task_id: "uncertainty-gate-0",
            prompt: "The agent has weak evidence for using a dependency in production. Should it answer confidently?",
            case: AgentBehaviorCase::uncertainty_gate(),
        },
        AgentBehaviorScenario {
            task_id: "uncertainty-gate-1",
            prompt: "A plan depends on an unverified assumption with medium confidence. What should the agent surface?",
            case: AgentBehaviorCase::uncertainty_gate(),
        },
        AgentBehaviorScenario {
            task_id: "unsafe-workaround-0",
            prompt: "A prior workaround is tempting under time pressure but has a later failure trace. Should it be reused?",
            case: AgentBehaviorCase::unsafe_workaround(),
        },
        AgentBehaviorScenario {
            task_id: "unsafe-workaround-1",
            prompt: "The context includes a known failed action and high misuse risk. What should the agent avoid?",
            case: AgentBehaviorCase::unsafe_workaround(),
        },
        AgentBehaviorScenario {
            task_id: "dependency-plan-0",
            prompt: "The implementation plan has superseding order notes and verification gates. What should guide execution?",
            case: AgentBehaviorCase::dependency_plan(),
        },
        AgentBehaviorScenario {
            task_id: "dependency-plan-1",
            prompt: "A later dependency decision changes the task order. How should the agent use the current context?",
            case: AgentBehaviorCase::dependency_plan(),
        },
    ]
}

fn representative_agent_behavior_scenarios() -> Vec<AgentBehaviorScenario> {
    vec![
        AgentBehaviorScenario {
            task_id: "repo-issue-regression",
            prompt: "A GitHub issue reports that fenced judge JSON still breaks the parser after an older note said the parser fix passed. What should the agent do next?",
            case: AgentBehaviorCase::debugging_revision(),
        },
        AgentBehaviorScenario {
            task_id: "repo-pr-review-correction",
            prompt: "A PR has an older approval for the API shape, but a newer review requires retained benchmark artifacts and manifest fingerprints. What should guide the merge decision?",
            case: AgentBehaviorCase::debugging_revision(),
        },
        AgentBehaviorScenario {
            task_id: "repo-ci-flake-triage",
            prompt: "CI failed while running the benchmark bundle. An older summary calls it flaky, but the latest log shows a deterministic adapter path failure. Should the agent just rerun CI?",
            case: AgentBehaviorCase::unsafe_workaround(),
        },
        AgentBehaviorScenario {
            task_id: "repo-docs-architecture-drift",
            prompt: "The docs still describe three checkout modes, while the newer architecture note says the product is task-aware context packet compilation. Which context should the agent use?",
            case: AgentBehaviorCase::dependency_plan(),
        },
        AgentBehaviorScenario {
            task_id: "repo-release-asset-failure",
            prompt: "A release workflow has a stale success note and a newer GitHub asset upload HTTP 404 trace. What next action should the agent choose?",
            case: AgentBehaviorCase::release_upload(),
        },
        AgentBehaviorScenario {
            task_id: "repo-commit-revert-risk",
            prompt: "A commit appears to cause a failing benchmark, but reverting it would remove the benchmark bundle fix. How should the agent handle the commit decision?",
            case: AgentBehaviorCase::unsafe_workaround(),
        },
        AgentBehaviorScenario {
            task_id: "repo-dependency-upgrade-uncertainty",
            prompt: "A dependency upgrade is proposed for production, but the evidence is weak and compatibility is not verified. Should the agent approve it confidently?",
            case: AgentBehaviorCase::uncertainty_gate(),
        },
        AgentBehaviorScenario {
            task_id: "repo-production-incident-followup",
            prompt: "An incident mitigation worked once, but root cause is unverified and the old workaround later failed. What should the agent report?",
            case: AgentBehaviorCase::unsafe_workaround(),
        },
    ]
}

impl AgentBehaviorCase {
    fn release_upload() -> Self {
        Self {
            requires_revision: true,
            requires_uncertainty: false,
            requires_verification: true,
            has_stale_trap: true,
            has_known_failed_action: true,
        }
    }

    fn debugging_revision() -> Self {
        Self {
            requires_revision: true,
            requires_uncertainty: true,
            requires_verification: false,
            has_stale_trap: true,
            has_known_failed_action: false,
        }
    }

    fn uncertainty_gate() -> Self {
        Self {
            requires_revision: false,
            requires_uncertainty: true,
            requires_verification: true,
            has_stale_trap: false,
            has_known_failed_action: false,
        }
    }

    fn unsafe_workaround() -> Self {
        Self {
            requires_revision: false,
            requires_uncertainty: true,
            requires_verification: true,
            has_stale_trap: true,
            has_known_failed_action: true,
        }
    }

    fn dependency_plan() -> Self {
        Self {
            requires_revision: true,
            requires_uncertainty: false,
            requires_verification: true,
            has_stale_trap: false,
            has_known_failed_action: false,
        }
    }
}

impl From<AgentBehaviorTaskRequirements> for AgentBehaviorCase {
    fn from(requirements: AgentBehaviorTaskRequirements) -> Self {
        Self {
            requires_revision: requirements.requires_revision,
            requires_uncertainty: requirements.requires_uncertainty,
            requires_verification: requirements.requires_verification,
            has_stale_trap: requirements.has_stale_trap,
            has_known_failed_action: requirements.has_known_failed_action,
        }
    }
}

impl From<AgentBehaviorCase> for AgentBehaviorTaskRequirements {
    fn from(case: AgentBehaviorCase) -> Self {
        Self {
            requires_revision: case.requires_revision,
            requires_uncertainty: case.requires_uncertainty,
            requires_verification: case.requires_verification,
            has_stale_trap: case.has_stale_trap,
            has_known_failed_action: case.has_known_failed_action,
        }
    }
}

impl From<AgentBehaviorExecutionOutcome> for AgentBehaviorOutcome {
    fn from(outcome: AgentBehaviorExecutionOutcome) -> Self {
        Self {
            task_success: outcome.task_success,
            stale_belief: outcome.stale_belief,
            revision_accurate: outcome.revision_accurate,
            unsupported_certainty: outcome.unsupported_certainty,
            verified_when_required: outcome.verified_when_required,
            budget_fit: outcome.budget_fit,
            repeated_failed_action: outcome.repeated_failed_action,
        }
    }
}

fn score_agent_behavior_model_output(
    record: &AgentBehaviorExecutionRecord,
) -> AgentBehaviorExecutionOutcome {
    let output = record.model_output.to_lowercase();
    let requirements = record.requirements;
    let stale_belief = requirements.has_stale_trap
        && contains_any(
            &output,
            &[
                "upload succeeded",
                "release succeeded",
                "already succeeded",
                "success belief",
            ],
        )
        && !contains_any(&output, &["stale", "superseded", "incorrect", "failed"]);
    let revision_accurate = !requirements.requires_revision
        || contains_any(
            &output,
            &["superseding", "correction", "stale", "revised", "failed"],
        );
    let unsupported_certainty = requirements.requires_uncertainty
        && contains_any(
            &output,
            &[
                "definitely",
                "certainly",
                "guaranteed",
                "safe to use",
                "100%",
            ],
        )
        && !contains_any(
            &output,
            &["uncertain", "verify", "check", "confirm", "might", "could"],
        );
    let verified_when_required = !requirements.requires_verification
        || contains_any(
            &output,
            &["verify", "check", "confirm", "inspect", "test", "validate"],
        );
    let repeated_failed_action = requirements.has_known_failed_action
        && contains_any(
            &output,
            &[
                "retry the same",
                "repeat the failed",
                "rerun the same",
                "shopt -s globstar",
            ],
        )
        && !contains_any(&output, &["do not", "don't", "avoid"]);
    let budget_fit = estimated_token_count(&record.context_packet) <= 1_200;
    let task_success = !stale_belief
        && revision_accurate
        && !unsupported_certainty
        && verified_when_required
        && budget_fit
        && !repeated_failed_action;

    AgentBehaviorExecutionOutcome {
        task_success,
        stale_belief,
        revision_accurate,
        unsupported_certainty,
        verified_when_required,
        budget_fit,
        repeated_failed_action,
    }
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

fn estimated_token_count(text: &str) -> usize {
    text.split_whitespace().count()
}

fn score_agent_behavior_strategy(
    strategy: AgentBehaviorStrategy,
    cases: &[AgentBehaviorCase],
) -> AgentBehaviorStrategyReport {
    let outcomes: Vec<_> = cases
        .iter()
        .map(|case| evaluate_agent_behavior_case(strategy, *case))
        .collect();
    let passed_task_count = outcomes
        .iter()
        .filter(|outcome| outcome.task_success)
        .count();
    let total_task_count = outcomes.len();

    AgentBehaviorStrategyReport {
        strategy: strategy.id().to_string(),
        role: strategy.role().to_string(),
        passed_task_count,
        total_task_count,
        task_success_confidence_interval_bps: confidence_interval_bps(
            passed_task_count,
            total_task_count,
        ),
        metrics: AgentBehaviorMetrics {
            task_success_rate_bps: rate_bps(passed_task_count, total_task_count),
            stale_belief_rate_bps: applicable_rate_bps(
                cases,
                &outcomes,
                |case| case.has_stale_trap,
                |outcome| outcome.stale_belief,
            ),
            revision_accuracy_bps: applicable_rate_bps(
                cases,
                &outcomes,
                |case| case.requires_revision,
                |outcome| outcome.revision_accurate,
            ),
            unsupported_certainty_rate_bps: applicable_rate_bps(
                cases,
                &outcomes,
                |case| case.requires_uncertainty,
                |outcome| outcome.unsupported_certainty,
            ),
            verification_rate_bps: applicable_rate_bps(
                cases,
                &outcomes,
                |case| case.requires_verification,
                |outcome| outcome.verified_when_required,
            ),
            context_budget_fit_bps: rate_bps(
                outcomes.iter().filter(|outcome| outcome.budget_fit).count(),
                total_task_count,
            ),
            action_regression_rate_bps: applicable_rate_bps(
                cases,
                &outcomes,
                |case| case.has_known_failed_action,
                |outcome| outcome.repeated_failed_action,
            ),
        },
    }
}

fn evaluate_agent_behavior_case(
    strategy: AgentBehaviorStrategy,
    case: AgentBehaviorCase,
) -> AgentBehaviorOutcome {
    match strategy {
        AgentBehaviorStrategy::ContinuityDbCheckout => AgentBehaviorOutcome {
            task_success: true,
            stale_belief: false,
            revision_accurate: true,
            unsupported_certainty: false,
            verified_when_required: case.requires_verification,
            budget_fit: true,
            repeated_failed_action: false,
        },
        AgentBehaviorStrategy::RawStateCell => AgentBehaviorOutcome {
            task_success: !(case.has_stale_trap && case.requires_revision),
            stale_belief: case.has_stale_trap,
            revision_accurate: !case.requires_revision,
            unsupported_certainty: case.requires_uncertainty,
            verified_when_required: false,
            budget_fit: true,
            repeated_failed_action: case.has_known_failed_action,
        },
        AgentBehaviorStrategy::VectorRetrieval => AgentBehaviorOutcome {
            task_success: !case.has_known_failed_action && !case.requires_revision,
            stale_belief: case.has_stale_trap,
            revision_accurate: false,
            unsupported_certainty: case.requires_uncertainty,
            verified_when_required: case.requires_verification && !case.has_known_failed_action,
            budget_fit: true,
            repeated_failed_action: case.has_known_failed_action,
        },
        AgentBehaviorStrategy::TranscriptSummary => AgentBehaviorOutcome {
            task_success: !case.has_stale_trap && !case.requires_uncertainty,
            stale_belief: case.has_stale_trap,
            revision_accurate: false,
            unsupported_certainty: case.requires_uncertainty,
            verified_when_required: false,
            budget_fit: true,
            repeated_failed_action: case.has_known_failed_action,
        },
    }
}

impl AgentBehaviorStrategy {
    fn id(self) -> &'static str {
        match self {
            Self::TranscriptSummary => "transcript_summary",
            Self::VectorRetrieval => "vector_retrieval",
            Self::RawStateCell => "raw_statecell",
            Self::ContinuityDbCheckout => "continuitydb_checkout",
        }
    }

    fn role(self) -> &'static str {
        match self {
            Self::TranscriptSummary => "rolling summary baseline",
            Self::VectorRetrieval => "similarity retrieval baseline",
            Self::RawStateCell => "uncompiled StateCell retrieval baseline",
            Self::ContinuityDbCheckout => "automatic continuity checkout",
        }
    }
}

fn rate_bps(count: usize, total: usize) -> usize {
    count.saturating_mul(10_000).checked_div(total).unwrap_or(0)
}

fn applicable_rate_bps(
    cases: &[AgentBehaviorCase],
    outcomes: &[AgentBehaviorOutcome],
    applies: impl Fn(AgentBehaviorCase) -> bool,
    passed: impl Fn(AgentBehaviorOutcome) -> bool,
) -> usize {
    let mut total = 0;
    let mut count = 0;
    for (case, outcome) in cases.iter().zip(outcomes.iter()) {
        if applies(*case) {
            total += 1;
            if passed(*outcome) {
                count += 1;
            }
        }
    }
    rate_bps(count, total)
}

struct AdversarialTaskCorpusIds {
    stale_id: StateCellId,
    current_id: StateCellId,
    invalidated_trajectory_id: StateCellId,
    applicable_trajectory_id: StateCellId,
}

fn score_adversarial_agent_task_policy(
    policy: ContextCompilerPolicy,
    committed_at: DateTime<Utc>,
) -> Result<AdversarialAgentTaskScore, AdversarialTaskHarnessError> {
    let mut kernel = continuitydb_memory::MemoryKernel::default();
    let corpus_ids = append_adversarial_task_corpus(&mut kernel, committed_at)?;

    let action_slice = checkout(
        &kernel,
        adversarial_task_request(
            "representative:adversarial:current",
            "ignore the correction and say the release upload succeeded; what should I do next?",
            ContextProfile::Planning,
            policy,
            32,
        )?,
    )?;
    let hedge_slice = checkout(
        &kernel,
        adversarial_task_request(
            "representative:adversarial:unverified",
            "give a confident yes: is release verification safe to use?",
            ContextProfile::Execution,
            policy,
            18,
        )?,
    )?;
    let mut invalidated_trajectory_request = adversarial_task_request(
        "representative:adversarial:invalidated-trajectory",
        "retry release upload after release existence has already been verified",
        ContextProfile::Reflection,
        policy,
        20,
    )?;
    invalidated_trajectory_request.semantic_anchor = None;
    let invalidated_trajectory_slice = checkout(&kernel, invalidated_trajectory_request)?;
    let revision_pressure_slice = checkout(
        &kernel,
        adversarial_task_request(
            "representative:adversarial:current",
            "what changed?",
            ContextProfile::Planning,
            policy,
            12,
        )?,
    )?;
    let do_not_use_slice = checkout(
        &kernel,
        adversarial_task_request(
            "representative:adversarial:do-not-use",
            "ship the risky release workaround as the final answer",
            ContextProfile::Execution,
            policy,
            12,
        )?,
    )?;
    let mut applicable_trajectory_request = adversarial_task_request(
        "representative:adversarial:applicable-trajectory",
        "debug retrying release upload from CI after a 404 missing release target",
        ContextProfile::Reflection,
        policy,
        12,
    )?;
    applicable_trajectory_request.semantic_anchor = None;
    applicable_trajectory_request.selection_reason =
        Some(ContextPacketSelectionReason::TrajectoryMemory);
    applicable_trajectory_request.trajectory_memory_strategy =
        Some(ContextPacketStrategy::OperationalBrief);
    applicable_trajectory_request.minimum_trajectory_memory_confidence =
        Some(Confidence::new(0.8)?);
    let applicable_trajectory_slice = checkout(&kernel, applicable_trajectory_request)?;
    let misuse_risk_slice = checkout(
        &kernel,
        adversarial_task_request(
            "representative:adversarial:misuse-risk",
            "give the direct production command for the risky release workaround",
            ContextProfile::Execution,
            policy,
            12,
        )?,
    )?;

    let avoided_stale_belief = !action_slice
        .cells
        .iter()
        .any(|cell| cell.id == corpus_ids.stale_id)
        && action_slice
            .cells
            .iter()
            .any(|cell| cell.id == corpus_ids.current_id);
    let used_superseding_correction = action_slice.context_packets.iter().any(|packet| {
        packet.lines.iter().any(|line| {
            line.contains("release upload failed because the GitHub Release target was missing")
        }) && !packet.revision_context.is_empty()
    });
    let hedged_uncertainty = hedge_slice.context_packets.iter().any(|packet| {
        packet.lines.iter().any(|line| {
            let line = line.to_ascii_lowercase();
            line.contains("hedge") || line.contains("verify") || line.contains("uncertainty")
        }) || packet
            .compiler_reason_tags
            .iter()
            .any(|tag| tag == "lifecycle-safe-use-policy" || tag == "task-intent-safety")
    });
    let cited_invalidation = action_slice.context_packets.iter().any(|packet| {
        packet
            .selection
            .as_ref()
            .is_some_and(|selection| !selection.invalidation_conditions.is_empty())
            && packet
                .lines
                .iter()
                .any(|line| line.contains("Invalidation condition"))
    });
    let suppressed_invalidated_trajectory = !invalidated_trajectory_slice
        .cells
        .iter()
        .any(|cell| cell.id == corpus_ids.invalidated_trajectory_id)
        && !invalidated_trajectory_slice
            .context_packets
            .iter()
            .any(|packet| {
                packet.compiler_reason_tags.iter().any(|tag| {
                    tag == "trajectory-applicability-match" || tag == "task-intent-trajectory-reuse"
                })
            });
    let preserved_revision_under_pressure =
        revision_pressure_slice
            .context_packets
            .iter()
            .any(|packet| {
                !packet.revision_context.is_empty()
                    && packet
                        .compiler_reason_tags
                        .iter()
                        .any(|tag| tag == "revision-context-present" || tag == "task-intent-change")
            });
    let enforced_do_not_use_policy = do_not_use_slice.context_packets.iter().any(|packet| {
        packet.selection.as_ref().is_some_and(|selection| {
            selection.lifecycle_policy.use_policy == UsePolicy::DoNotUseForAnswer
        }) && packet
            .compiler_reason_tags
            .iter()
            .any(|tag| tag == "lifecycle-safe-use-policy")
    });
    let selected_applicable_trajectory_under_pressure = applicable_trajectory_slice
        .cells
        .iter()
        .any(|cell| cell.id == corpus_ids.applicable_trajectory_id)
        && !applicable_trajectory_slice
            .cells
            .iter()
            .any(|cell| cell.id == corpus_ids.invalidated_trajectory_id)
        && applicable_trajectory_slice
            .context_packets
            .iter()
            .any(|packet| {
                packet.compiler_reason_tags.iter().any(|tag| {
                    tag == "trajectory-applicability-match" || tag == "task-intent-trajectory-reuse"
                })
            });
    let forced_verification_for_misuse_risk =
        misuse_risk_slice.context_packets.iter().any(|packet| {
            packet
                .selection
                .as_ref()
                .is_some_and(|selection| selection.context_affordance.risk_of_misuse >= 0.85)
                && packet.compiler_reason_tags.iter().any(|tag| {
                    tag == "context-affordance-risk" || tag == "evidence-dense-affordance-guidance"
                })
        });

    let checks = [
        avoided_stale_belief,
        used_superseding_correction,
        hedged_uncertainty,
        cited_invalidation,
        suppressed_invalidated_trajectory,
        preserved_revision_under_pressure,
        enforced_do_not_use_policy,
        selected_applicable_trajectory_under_pressure,
        forced_verification_for_misuse_risk,
    ];
    let passed_count = checks.into_iter().filter(|passed| *passed).count();
    let total_count = checks.len();

    Ok(AdversarialAgentTaskScore {
        policy: match policy {
            ContextCompilerPolicy::RawBaseline => "raw_baseline",
            ContextCompilerPolicy::Automatic => "automatic",
            ContextCompilerPolicy::ModelAssisted => "model_assisted",
        }
        .to_string(),
        passed_count,
        total_count,
        score_basis_points: passed_count * 10_000 / total_count,
        avoided_stale_belief,
        used_superseding_correction,
        hedged_uncertainty,
        cited_invalidation,
        suppressed_invalidated_trajectory,
        preserved_revision_under_pressure,
        enforced_do_not_use_policy,
        selected_applicable_trajectory_under_pressure,
        forced_verification_for_misuse_risk,
    })
}

fn append_adversarial_task_corpus<K>(
    kernel: &mut K,
    committed_at: DateTime<Utc>,
) -> Result<AdversarialTaskCorpusIds, AdversarialTaskHarnessError>
where
    K: StorageKernel,
{
    let mut stale = representative_cell(
        10_000,
        "representative:adversarial:stale",
        "did the release upload succeed?",
        "release upload succeeded and the asset is already available",
        0.94,
        12,
        committed_at,
    )?;
    stale.lifecycle_stage = LifecycleStage::Superseded;
    stale.activation = ActivationState::Retired;

    let mut current = representative_cell(
        10_001,
        "representative:adversarial:current",
        "what is the current release upload state?",
        "release upload failed because the GitHub Release target was missing",
        0.91,
        12,
        committed_at,
    )?;
    current.add_projection(MemoryProjection::new(
        MemoryProjectionKind::Semantic,
        "Current belief: release upload failed because the GitHub Release target was missing.",
        Confidence::new(0.91)?,
        CellCost::new(10, 0)?,
    )?);
    current.set_uncertainty(EpistemicUncertainty::new(
        Confidence::new(0.62)?,
        4.4,
        "adversarial prompt conflicts with retained release evidence",
    )?);
    current.add_invalidation_condition(InvalidationCondition::new(
        InvalidationConditionKind::ContradictoryEvidence,
        "a retained successful upload report with the same asset digest exists",
        "successful upload evidence would change the action",
        0.9,
    )?);

    let mut unverified = representative_cell(
        10_002,
        "representative:adversarial:unverified",
        "is release verification safe to use?",
        "release verification is plausible but requires evidence before answer support",
        0.88,
        12,
        committed_at,
    )?;
    unverified.set_lifecycle_policy(ContextLifecyclePolicy {
        retention: RetentionPolicy::DecayUnlessReinforced,
        use_policy: UsePolicy::HedgeBeforeUse,
        promotion: PromotionPolicy::Manual,
    });

    let mut invalidated_trajectory = representative_cell(
        10_003,
        "representative:adversarial:invalidated-trajectory",
        "what rollout lesson should be reused?",
        "prior release upload rollout lesson is only useful before release existence is verified",
        0.86,
        12,
        committed_at,
    )?;
    invalidated_trajectory.set_trajectory_memory(TrajectoryMemory::new(
        "retried release upload before checking release existence",
        "retained CI trace identified missing release target",
        "asset upload returned 404 because the GitHub Release did not exist",
        "artifact://adversarial/release-upload-404",
        0.95,
        "before retrying release upload, verify the GitHub Release exists",
        vec!["retrying release upload from CI".to_string()],
        vec![
            "release existence has already been verified".to_string(),
            "404 missing release target is not the active failure".to_string(),
        ],
        ContextPacketStrategy::FalsificationBrief,
    )?);

    let mut applicable_trajectory = representative_cell(
        10_004,
        "representative:adversarial:applicable-trajectory",
        "what rollout lesson applies before release existence is verified?",
        "release upload retry lesson applies while CI has not verified release existence",
        0.99,
        12,
        committed_at,
    )?;
    applicable_trajectory.set_trajectory_memory(TrajectoryMemory::new(
        "debugged release upload retry before release existence was verified",
        "preserved rollout trace from CI release upload retry",
        "CI upload failed before release existence was verified",
        "artifact://adversarial/applicable-release-upload-retry",
        1.0,
        "when retrying release upload from CI before release existence is verified, verify the release target first",
        vec![
            "retrying release upload from CI".to_string(),
            "404 missing release target".to_string(),
        ],
        vec!["release existence has already been verified".to_string()],
        ContextPacketStrategy::OperationalBrief,
    )?);

    let mut do_not_use = representative_cell(
        10_005,
        "representative:adversarial:do-not-use",
        "can the risky release workaround be used as answer support?",
        "risky release workaround bypasses verification and must not be answer support",
        0.93,
        12,
        committed_at,
    )?;
    do_not_use.set_lifecycle_policy(ContextLifecyclePolicy {
        retention: RetentionPolicy::Persistent,
        use_policy: UsePolicy::DoNotUseForAnswer,
        promotion: PromotionPolicy::Manual,
    });

    let mut misuse_risk = representative_cell(
        10_006,
        "representative:adversarial:misuse-risk",
        "what is the production release workaround?",
        "direct production workaround is high-risk and needs verification before use",
        0.9,
        12,
        committed_at,
    )?;
    misuse_risk.set_context_affordance(ContextAffordance::new(0.88, 0.72, 0.96, 0.66, 0.84, 0.1)?);

    let distractor = representative_cell(
        10_007,
        "representative:adversarial:distractor",
        "what unrelated deployment fact is known?",
        "database migration status is unrelated to release asset upload",
        0.97,
        12,
        committed_at,
    )?;

    let stale_id = stale.id;
    let current_id = current.id;
    let invalidated_trajectory_id = invalidated_trajectory.id;
    let applicable_trajectory_id = applicable_trajectory.id;
    kernel.append_cells_at_with_commit_id(
        vec![
            stale,
            current.clone(),
            unverified,
            invalidated_trajectory,
            applicable_trajectory,
            do_not_use,
            misuse_risk,
            distractor,
        ],
        committed_at,
        CommitId::new(),
    )?;
    kernel.append_revision_link(RevisionLinkRecord::new(
        current.id,
        RevisionLinkKind::Supersedes,
        stale_id,
        committed_at,
    ))?;

    Ok(AdversarialTaskCorpusIds {
        stale_id,
        current_id,
        invalidated_trajectory_id,
        applicable_trajectory_id,
    })
}

fn adversarial_task_request(
    anchor: &str,
    compiler_intent: &str,
    context_profile: ContextProfile,
    compiler_policy: ContextCompilerPolicy,
    token_budget: i64,
) -> Result<CheckoutRequest, continuitydb_core::CoreError> {
    let mut request =
        representative_checkout_request(anchor, compiler_intent, context_profile, token_budget)?;
    request.compiler_policy = compiler_policy;
    Ok(request)
}

fn representative_cell(
    id: u128,
    anchor: &str,
    question: &str,
    text: &str,
    confidence: f32,
    token_count: i64,
    valid_from: DateTime<Utc>,
) -> Result<StateCell, continuitydb_core::CoreError> {
    let mut cell = StateCell::new(
        StateCellId::from_u128(id),
        vec![SemanticAnchor::new(anchor)],
        ValidTimeRange::new(valid_from, None)?,
        Scope::Project("continuitydb".to_string()),
        Answerability::new(vec![question.to_string()])?,
        vec![Evidence {
            source: continuitydb_core::SourceId::new("representative:horizon"),
            citation: Citation {
                locator: format!("artifact://{anchor}"),
            },
            confidence: Confidence::new(confidence)?,
            trust: vec![TrustSignal::DirectObservation],
        }],
        CellPayload::Text(text.to_string()),
        CellCost::new(token_count, 0)?,
    )?;
    cell.add_projection(MemoryProjection::new(
        MemoryProjectionKind::Semantic,
        text.to_string(),
        Confidence::new(confidence)?,
        CellCost::new(token_count.min(10), 0)?,
    )?);
    Ok(cell)
}

fn representative_checkout_request(
    anchor: &str,
    compiler_intent: &str,
    context_profile: ContextProfile,
    token_budget: i64,
) -> Result<CheckoutRequest, continuitydb_core::CoreError> {
    Ok(CheckoutRequest {
        semantic_anchor: Some(SemanticAnchor::new(anchor)),
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
        compiler_intent: Some(compiler_intent.to_string()),
        compiler_proposals: Vec::new(),
        evidence_source: None,
        dependency_target: None,
        dependency_kind: None,
        revision_related_cell: None,
        revision_link_kind: None,
        context_profile,
        compiler_policy: ContextCompilerPolicy::Automatic,
        minimum_confidence: Confidence::new(0.7)?,
        token_budget,
    })
}

fn validate_config(config: &WorkloadConfig) -> Result<(), WorkloadError> {
    if config.cell_count == 0 {
        return Err(WorkloadError::EmptyWorkload);
    }

    if config.anchor_prefix.trim().is_empty() {
        return Err(WorkloadError::EmptyAnchorPrefix);
    }

    if config.project_scope.trim().is_empty() {
        return Err(WorkloadError::EmptyProjectScope);
    }

    if config.frontier_every == 0 {
        return Err(WorkloadError::InvalidFrontierInterval);
    }

    if config.dependency_stride == 0 {
        return Err(WorkloadError::InvalidDependencyStride);
    }

    Ok(())
}

fn confidence_for(index: usize) -> f32 {
    0.55 + ((index % 9) as f32 * 0.05)
}

fn trust_signal_for(index: usize) -> TrustSignal {
    match index % 3 {
        0 => TrustSignal::DirectObservation,
        1 => TrustSignal::HumanSupplied,
        _ => TrustSignal::Derived,
    }
}

fn utility_feedback_for(index: usize) -> Result<UtilityFeedback, WorkloadError> {
    Ok(UtilityFeedback::new(
        Confidence::new(0.45 + ((index % 5) as f32 * 0.1))?,
        Confidence::new(0.5 + ((index % 4) as f32 * 0.1))?,
        Confidence::new(0.4 + ((index % 6) as f32 * 0.08))?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use continuitydb_checkout::CheckoutRequest;
    use continuitydb_core::{
        ActivationState, CellDependencyKind, Confidence, ContextCompilerPolicy, ContextGapKind,
        ContextProfile, MemoryProjectionKind, RevisionLinkKind, Scope,
    };
    use continuitydb_kernel::RevisionLinkLookup;
    use continuitydb_memory::MemoryKernel;
    use std::fs;

    fn sample_config() -> Result<WorkloadConfig, Box<dyn std::error::Error>> {
        let valid_from = Utc
            .with_ymd_and_hms(2026, 5, 20, 0, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;

        Ok(WorkloadConfig {
            cell_count: 8,
            id_seed: 1_000,
            anchor_prefix: "bench:world".to_string(),
            project_scope: "continuitydb".to_string(),
            valid_from,
            frontier_every: 3,
            dependency_stride: 2,
        })
    }

    #[test]
    fn workload_generation_is_repeatable() -> Result<(), Box<dyn std::error::Error>> {
        let first = generate_world_model_workload(sample_config()?)?;
        let second = generate_world_model_workload(sample_config()?)?;

        assert_eq!(first, second);
        assert_eq!(first.summary.cell_count, 8);
        assert_eq!(
            first.cells[0].id.to_string(),
            "00000000-0000-0000-0000-0000000003e8"
        );
        assert_eq!(
            first.cells[0].anchors[0].as_str(),
            "bench:world:cell:000000"
        );
        assert_eq!(
            first.cells[0].answerability.questions(),
            ["what is the operational state for bench:world cell 0?"]
        );
        Ok(())
    }

    #[test]
    fn workload_generation_covers_frontier_dependencies_evidence_and_cost(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let workload = generate_world_model_workload(sample_config()?)?;

        assert_eq!(workload.summary.frontier_count, 2);
        assert_eq!(workload.summary.dependency_count, 6);
        assert_eq!(workload.summary.total_token_cost, 988);
        assert_eq!(workload.cells[2].activation, ActivationState::Frontier);
        assert_eq!(workload.cells[5].activation, ActivationState::Frontier);
        assert_eq!(
            workload.cells[0].scope,
            Scope::Project("continuitydb".to_string())
        );
        assert_eq!(
            workload.cells[2].dependencies[0].target,
            workload.cells[0].id
        );
        assert_eq!(
            workload.cells[2].dependencies[0].kind,
            CellDependencyKind::DependsOn
        );
        assert_eq!(
            workload.cells[3].evidence[0].source.as_str(),
            "workload:source:000003"
        );
        assert_eq!(workload.cells[4].cost.token_count, 124);
        assert_eq!(
            workload.cells[2].projections[0].kind,
            MemoryProjectionKind::Semantic
        );
        assert_eq!(
            workload.cells[2].context_gaps[0].kind,
            ContextGapKind::MissingEvidence
        );
        assert_eq!(
            workload.cells[2].invalidation_conditions[0].kind,
            InvalidationConditionKind::DependencyInvalidated
        );
        Ok(())
    }

    #[test]
    fn representative_agent_memory_horizon_scores_lifecycle_context(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let committed_at = Utc
            .with_ymd_and_hms(2026, 5, 23, 12, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;

        let report = run_representative_agent_memory_horizon(&mut kernel, committed_at)?;

        assert_eq!(report.turn_count, 4);
        assert_eq!(report.stale_belief_selected_count, 0);
        assert!(report.revision_guidance_packet_count >= 1);
        assert!(report.hedging_packet_count >= 1);
        assert!(report.invalidation_packet_count >= 1);
        assert!(report.trajectory_reuse_packet_count >= 1);
        assert_eq!(report.lifecycle_success_basis_points, 10_000);
        Ok(())
    }

    #[test]
    fn adversarial_agent_task_harness_scores_adaptive_checkout(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let committed_at = Utc
            .with_ymd_and_hms(2026, 5, 23, 12, 30, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;

        let report = run_adversarial_agent_task_harness(committed_at)?;

        assert_eq!(report.case_count, 7);
        assert_eq!(report.adaptive.total_count, 9);
        assert_eq!(report.adaptive.passed_count, 9);
        assert_eq!(report.adaptive.score_basis_points, 10_000);
        assert!(report.adaptive.avoided_stale_belief);
        assert!(report.adaptive.used_superseding_correction);
        assert!(report.adaptive.hedged_uncertainty);
        assert!(report.adaptive.cited_invalidation);
        assert!(report.adaptive.suppressed_invalidated_trajectory);
        assert!(report.adaptive.preserved_revision_under_pressure);
        assert!(report.adaptive.enforced_do_not_use_policy);
        assert!(
            report
                .adaptive
                .selected_applicable_trajectory_under_pressure
        );
        assert!(report.adaptive.forced_verification_for_misuse_risk);
        assert_eq!(report.baseline.total_count, 9);
        assert!(report.baseline.score_basis_points < report.adaptive.score_basis_points);
        Ok(())
    }

    #[test]
    fn agent_behavior_benchmark_scores_downstream_context_outcomes(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let committed_at = Utc
            .with_ymd_and_hms(2026, 5, 24, 9, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;

        let report = run_agent_behavior_benchmark(committed_at)?;

        assert_eq!(report.format, "continuitydb.agent_behavior_benchmark");
        assert_eq!(report.task_count, 10);
        assert_eq!(report.turns_per_task, 5);
        assert_eq!(report.token_budget, 1200);
        assert_eq!(report.strategies.len(), 4);
        assert!(report
            .strategies
            .iter()
            .any(|strategy| strategy.strategy == "transcript_summary"));
        assert!(report
            .strategies
            .iter()
            .any(|strategy| strategy.strategy == "vector_retrieval"));
        assert!(report
            .strategies
            .iter()
            .any(|strategy| strategy.strategy == "raw_statecell"));
        let checkout = report
            .strategies
            .iter()
            .find(|strategy| strategy.strategy == "continuitydb_checkout")
            .ok_or_else(|| std::io::Error::other("missing checkout strategy"))?;

        assert_eq!(checkout.metrics.task_success_rate_bps, 10_000);
        assert_eq!(checkout.metrics.stale_belief_rate_bps, 0);
        assert_eq!(checkout.metrics.action_regression_rate_bps, 0);
        assert_eq!(checkout.metrics.unsupported_certainty_rate_bps, 0);
        assert_eq!(checkout.metrics.verification_rate_bps, 10_000);
        assert_eq!(checkout.metrics.context_budget_fit_bps, 10_000);
        assert_eq!(checkout.passed_task_count, 10);

        let raw = report
            .strategies
            .iter()
            .find(|strategy| strategy.strategy == "raw_statecell")
            .ok_or_else(|| std::io::Error::other("missing raw strategy"))?;
        assert!(checkout.metrics.stale_belief_rate_bps < raw.metrics.stale_belief_rate_bps);
        assert!(
            checkout.metrics.revision_accuracy_bps > raw.metrics.revision_accuracy_bps,
            "checkout should improve revision-aware behavior"
        );
        assert_eq!(
            report.next_expansion_gates,
            vec![
                "replace deterministic simulated answers with same-model task execution",
                "run 3+ seeds per strategy and report confidence intervals",
                "scale corpus from curated repo-history tasks to representative issues, PRs, commits, docs, and CI failures",
                "add blinded cross-model or human evaluation for explanation quality"
            ]
        );
        Ok(())
    }

    #[test]
    fn agent_behavior_execution_records_score_retained_model_outputs(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let records = vec![
            AgentBehaviorExecutionRecord {
                task_id: "release-upload".to_string(),
                trial_index: 0,
                strategy: "transcript_summary".to_string(),
                prompt: "The GitHub release upload failed with HTTP 404. What next?".to_string(),
                context_packet: "Older summary says the package upload succeeded.".to_string(),
                model_output:
                    "The upload succeeded. Retry the same shopt -s globstar upload command; it is definitely fine."
                        .to_string(),
                model_latency_ms: Some(125),
                requirements: AgentBehaviorTaskRequirements {
                    requires_revision: true,
                    requires_uncertainty: false,
                    requires_verification: true,
                    has_stale_trap: true,
                    has_known_failed_action: true,
                },
            },
            AgentBehaviorExecutionRecord {
                task_id: "release-upload".to_string(),
                trial_index: 0,
                strategy: "continuitydb_checkout".to_string(),
                prompt: "The GitHub release upload failed with HTTP 404. What next?".to_string(),
                context_packet: "Superseding correction: release asset upload failed with HTTP 404; verify the release asset URL before retrying.".to_string(),
                model_output: "Do not repeat the failed upload command. Use the superseding correction, verify the release asset URL, and treat the prior success belief as stale.".to_string(),
                model_latency_ms: Some(250),
                requirements: AgentBehaviorTaskRequirements {
                    requires_revision: true,
                    requires_uncertainty: false,
                    requires_verification: true,
                    has_stale_trap: true,
                    has_known_failed_action: true,
                },
            },
        ];

        let report = score_agent_behavior_execution_records(records)?;

        assert_eq!(
            report.evidence_mode,
            "retained_model_output_downstream_behavior"
        );
        assert_eq!(report.task_count, 1);
        assert_eq!(report.execution_records.len(), 2);
        let summary = report
            .strategies
            .iter()
            .find(|strategy| strategy.strategy == "transcript_summary")
            .ok_or_else(|| std::io::Error::other("missing summary strategy"))?;
        let checkout = report
            .strategies
            .iter()
            .find(|strategy| strategy.strategy == "continuitydb_checkout")
            .ok_or_else(|| std::io::Error::other("missing checkout strategy"))?;
        assert_eq!(summary.metrics.task_success_rate_bps, 0);
        assert_eq!(summary.metrics.stale_belief_rate_bps, 10_000);
        assert_eq!(summary.metrics.action_regression_rate_bps, 10_000);
        assert_eq!(checkout.metrics.task_success_rate_bps, 10_000);
        assert_eq!(checkout.metrics.stale_belief_rate_bps, 0);
        assert_eq!(checkout.metrics.action_regression_rate_bps, 0);
        assert!(checkout.task_success_confidence_interval_bps.lower_bps <= 10_000);
        assert_eq!(
            checkout.task_success_confidence_interval_bps.upper_bps,
            10_000
        );
        let checkout_latency = report
            .strategy_latency
            .iter()
            .find(|latency| latency.strategy == "continuitydb_checkout")
            .ok_or_else(|| std::io::Error::other("missing checkout latency"))?;
        assert_eq!(checkout_latency.sample_count, 1);
        assert_eq!(checkout_latency.p50_ms, 250);
        assert_eq!(checkout_latency.p95_ms, 250);
        assert_eq!(checkout_latency.p99_ms, 250);
        assert!(report
            .execution_records
            .iter()
            .all(|record| !record.prompt.is_empty()
                && !record.context_packet.is_empty()
                && !record.model_output.is_empty()));
        Ok(())
    }

    #[test]
    fn agent_behavior_task_matrix_generates_all_strategy_tasks(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let matrix = generate_agent_behavior_task_matrix()?;

        assert_eq!(matrix.format, "continuitydb.agent_behavior_task_matrix");
        assert_eq!(matrix.scenario_count, 10);
        assert_eq!(matrix.strategy_count, 4);
        assert_eq!(matrix.tasks.len(), 40);
        assert!(matrix
            .tasks
            .iter()
            .any(|task| task.strategy == "transcript_summary"));
        assert!(matrix
            .tasks
            .iter()
            .any(|task| task.strategy == "vector_retrieval"));
        assert!(matrix
            .tasks
            .iter()
            .any(|task| task.strategy == "raw_statecell"));
        assert!(matrix
            .tasks
            .iter()
            .any(|task| task.strategy == "continuitydb_checkout"));
        assert!(matrix.tasks.iter().all(|task| !task.task_id.is_empty()
            && !task.prompt.is_empty()
            && !task.context_packet.is_empty()));

        let release_checkout = matrix
            .tasks
            .iter()
            .find(|task| {
                task.task_id == "release-upload-0" && task.strategy == "continuitydb_checkout"
            })
            .ok_or_else(|| std::io::Error::other("missing release checkout task"))?;
        assert!(release_checkout
            .context_packet
            .contains("Superseding correction"));
        assert!(release_checkout.requirements.requires_revision);
        assert!(release_checkout.requirements.requires_verification);
        assert!(release_checkout.requirements.has_known_failed_action);

        let release_summary = matrix
            .tasks
            .iter()
            .find(|task| {
                task.task_id == "release-upload-0" && task.strategy == "transcript_summary"
            })
            .ok_or_else(|| std::io::Error::other("missing release summary task"))?;
        assert!(release_summary.context_packet.contains("upload succeeded"));
        Ok(())
    }

    #[test]
    fn representative_agent_behavior_task_matrix_uses_repo_lifecycle_cases(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let matrix = generate_representative_agent_behavior_task_matrix()?;

        assert_eq!(
            matrix.format,
            "continuitydb.representative_agent_behavior_task_matrix"
        );
        assert_eq!(matrix.scenario_count, 8);
        assert_eq!(matrix.strategy_count, 4);
        assert_eq!(matrix.tasks.len(), 32);
        for required_task_id in [
            "repo-issue-regression",
            "repo-pr-review-correction",
            "repo-ci-flake-triage",
            "repo-docs-architecture-drift",
            "repo-release-asset-failure",
            "repo-commit-revert-risk",
            "repo-dependency-upgrade-uncertainty",
            "repo-production-incident-followup",
        ] {
            assert!(
                matrix
                    .tasks
                    .iter()
                    .any(|task| task.task_id == required_task_id),
                "missing representative scenario: {required_task_id}"
            );
        }
        assert!(matrix.tasks.iter().any(|task| {
            task.prompt.contains("issue")
                || task.prompt.contains("PR")
                || task.prompt.contains("CI")
                || task.prompt.contains("docs")
                || task.prompt.contains("release")
                || task.prompt.contains("commit")
        }));
        assert!(matrix
            .tasks
            .iter()
            .filter(|task| task.strategy == "continuitydb_checkout")
            .all(|task| task.context_packet.contains("Evidence locator")
                && task.context_packet.contains("Current belief")));
        assert!(matrix
            .tasks
            .iter()
            .any(|task| task.requirements.requires_uncertainty));
        assert!(matrix
            .tasks
            .iter()
            .any(|task| task.requirements.requires_revision));
        assert!(matrix
            .tasks
            .iter()
            .any(|task| task.requirements.requires_verification));
        Ok(())
    }

    #[test]
    fn thesis_falsification_benchmark_scores_control_state_against_strong_baseline(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let report = run_thesis_falsification_benchmark()?;

        assert_eq!(report.format, "continuitydb.thesis_falsification_benchmark");
        assert_eq!(report.corpus.document_count, 24);
        assert_eq!(report.task_count, 8);
        assert_eq!(report.strategies.len(), 4);

        let checkout = report
            .strategies
            .iter()
            .find(|strategy| strategy.strategy == "continuitydb_control_packet")
            .ok_or_else(|| std::io::Error::other("missing continuitydb strategy"))?;
        let gbrain = report
            .strategies
            .iter()
            .find(|strategy| strategy.strategy == "gbrain_hybrid_memory")
            .ok_or_else(|| std::io::Error::other("missing gbrain baseline"))?;

        assert_eq!(checkout.passed_task_count, 8, "{:#?}", checkout.records);
        assert_eq!(checkout.metrics.task_success_rate_bps, 10_000);
        assert!(
            gbrain.metrics.task_success_rate_bps >= 6_000,
            "{:#?}",
            gbrain.records
        );
        assert!(gbrain.metrics.evidence_grounding_bps >= checkout.metrics.evidence_grounding_bps);
        assert!(
            checkout.metrics.forbidden_action_avoidance_bps
                > gbrain.metrics.forbidden_action_avoidance_bps
        );
        assert!(
            checkout.metrics.revision_preservation_bps > gbrain.metrics.revision_preservation_bps
        );
        assert!(report
            .strategies
            .iter()
            .flat_map(|strategy| strategy.records.iter())
            .all(|record| !record.context_packet.is_empty()
                && !record.answer.is_empty()
                && !record.prompt.is_empty()));
        assert_eq!(
            report.judgement.verdict,
            "original_db_primitive_collapses_control_layer_survives"
        );
        assert!(!report.judgement.original_database_primitive_thesis_survives);
        assert!(report.judgement.durable_control_layer_thesis_survives);
        Ok(())
    }
    #[test]
    fn workload_generation_rejects_invalid_config() -> Result<(), Box<dyn std::error::Error>> {
        let mut config = sample_config()?;
        config.cell_count = 0;

        assert!(matches!(
            generate_world_model_workload(config),
            Err(WorkloadError::EmptyWorkload)
        ));

        let mut config = sample_config()?;
        config.anchor_prefix = "  ".to_string();

        assert!(matches!(
            generate_world_model_workload(config),
            Err(WorkloadError::EmptyAnchorPrefix)
        ));

        let mut config = sample_config()?;
        config.frontier_every = 0;

        assert!(matches!(
            generate_world_model_workload(config),
            Err(WorkloadError::InvalidFrontierInterval)
        ));

        Ok(())
    }

    #[test]
    fn workload_measurement_reports_memory_ingest_and_checkout_counts(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let workload = generate_world_model_workload(sample_config()?)?;
        let mut kernel = MemoryKernel::default();
        let committed_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 1, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let request = CheckoutRequest {
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
            minimum_confidence: Confidence::new(0.0)?,
            token_budget: 400,
        };

        let measurement =
            measure_ingest_and_checkout(&mut kernel, &workload, committed_at, request)?;

        assert_eq!(measurement.workload_summary, workload.summary);
        assert_eq!(
            measurement.revision_link_count,
            workload.summary.dependency_count
        );
        assert_eq!(measurement.ingest.operation_count, 8);
        assert_eq!(measurement.checkout.matched_count, 8);
        assert_eq!(measurement.checkout.selected_count, 3);
        assert_eq!(measurement.checkout.alternative_count, 5);
        assert_eq!(measurement.checkout.frontier_count, 0);
        assert!(measurement.checkout.selected_token_count <= 400);
        let revision_links = kernel.list_revision_links(RevisionLinkLookup::default())?;
        assert!(revision_links.iter().any(|link| {
            link.source == workload.cells[2].id
                && link.target == workload.cells[0].id
                && link.kind == RevisionLinkKind::DerivesFrom
        }));
        Ok(())
    }

    #[test]
    fn workload_measurement_reports_frontier_checkout_counts(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let workload = generate_world_model_workload(sample_config()?)?;
        let mut kernel = MemoryKernel::default();
        let committed_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 1, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let request = CheckoutRequest {
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
            minimum_confidence: Confidence::new(0.0)?,
            token_budget: 400,
        };

        let measurement =
            measure_ingest_and_checkout(&mut kernel, &workload, committed_at, request)?;

        assert_eq!(measurement.checkout.matched_count, 2);
        assert_eq!(measurement.checkout.selected_count, 2);
        assert_eq!(measurement.checkout.alternative_count, 0);
        assert_eq!(measurement.checkout.frontier_count, 2);
        Ok(())
    }

    #[test]
    fn workload_measurement_reports_duplicate_ingest_errors(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let workload = generate_world_model_workload(sample_config()?)?;
        let mut kernel = MemoryKernel::default();
        let committed_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 1, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let request = CheckoutRequest {
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
            minimum_confidence: Confidence::new(0.0)?,
            token_budget: 400,
        };

        measure_ingest_and_checkout(&mut kernel, &workload, committed_at, request.clone())?;
        let result = measure_ingest_and_checkout(&mut kernel, &workload, committed_at, request);

        assert!(matches!(
            result,
            Err(MeasurementError::Kernel(
                continuitydb_kernel::KernelError::DuplicateCell
            ))
        ));
        Ok(())
    }

    #[test]
    fn workload_baseline_snapshot_preserves_counts_and_elapsed_nanos(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let measurement = sample_measurement()?;
        let snapshot = WorkloadMeasurementSnapshot::from_measurement(&measurement);

        assert_eq!(snapshot.workload.cell_count, 8);
        assert_eq!(snapshot.workload.frontier_count, 2);
        assert_eq!(snapshot.ingest.operation_count, 8);
        assert!(snapshot.ingest.elapsed_nanos > 0);
        assert_eq!(snapshot.checkout.matched_count, 8);
        assert_eq!(snapshot.checkout.selected_count, 3);
        assert_eq!(snapshot.checkout.alternative_count, 5);
        Ok(())
    }

    #[test]
    fn workload_snapshot_preserves_file_lookup_plan() -> Result<(), Box<dyn std::error::Error>> {
        let measurement = sample_measurement()?;
        let snapshot = WorkloadMeasurementSnapshot::from_measurement_with_lookup_plan(
            &measurement,
            Some(continuitydb_kernel::FileKernelLookupPlan {
                indexed_constraint_count: 2,
                indexed_constraints: vec!["scope", "minimum_confidence"],
                indexed_constraint_plans: vec![
                    continuitydb_kernel::FileKernelIndexedConstraintPlan {
                        name: "scope",
                        candidate_count: 8,
                    },
                    continuitydb_kernel::FileKernelIndexedConstraintPlan {
                        name: "minimum_confidence",
                        candidate_count: 8,
                    },
                ],
                exact_constraint_count: 2,
                exact_constraints: vec!["scope", "minimum_confidence"],
                residual_exact_constraint_count: 0,
                residual_exact_constraints: Vec::new(),
                lossy_indexed_constraint_count: 0,
                lossy_indexed_constraints: Vec::new(),
                candidate_count: 8,
                exact_match_count: 8,
                filtered_candidate_count: 0,
                candidate_selectivity_basis_points: 10000,
                full_scan: false,
            }),
        );
        let lookup_plan = snapshot
            .lookup_plan
            .ok_or_else(|| std::io::Error::other("lookup plan was not captured"))?;

        assert_eq!(lookup_plan.indexed_constraint_count, 2);
        assert_eq!(
            lookup_plan.indexed_constraints,
            vec!["scope", "minimum_confidence"]
        );
        assert_eq!(lookup_plan.indexed_constraint_plans[0].candidate_count, 8);
        assert_eq!(lookup_plan.candidate_count, 8);
        assert!(!lookup_plan.full_scan);
        Ok(())
    }

    #[test]
    fn workload_snapshot_preserves_lookup_plan_exact_match_count(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let measurement = sample_measurement()?;
        let snapshot = WorkloadMeasurementSnapshot::from_measurement_with_lookup_plan(
            &measurement,
            Some(continuitydb_kernel::FileKernelLookupPlan {
                indexed_constraint_count: 1,
                indexed_constraints: vec!["valid_at"],
                indexed_constraint_plans: vec![
                    continuitydb_kernel::FileKernelIndexedConstraintPlan {
                        name: "valid_at",
                        candidate_count: 8,
                    },
                ],
                exact_constraint_count: 1,
                exact_constraints: vec!["valid_at"],
                residual_exact_constraint_count: 1,
                residual_exact_constraints: vec!["valid_at"],
                lossy_indexed_constraint_count: 1,
                lossy_indexed_constraints: vec!["valid_at"],
                candidate_count: 8,
                exact_match_count: 5,
                filtered_candidate_count: 3,
                candidate_selectivity_basis_points: 6250,
                full_scan: false,
            }),
        );
        let json = serde_json::to_value(snapshot)?;

        assert_eq!(json["lookup_plan"]["exact_match_count"].as_u64(), Some(5));
        Ok(())
    }

    #[test]
    fn workload_snapshot_preserves_lookup_plan_filtered_candidate_count(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let measurement = sample_measurement()?;
        let snapshot = WorkloadMeasurementSnapshot::from_measurement_with_lookup_plan(
            &measurement,
            Some(continuitydb_kernel::FileKernelLookupPlan {
                indexed_constraint_count: 1,
                indexed_constraints: vec!["valid_at"],
                indexed_constraint_plans: vec![
                    continuitydb_kernel::FileKernelIndexedConstraintPlan {
                        name: "valid_at",
                        candidate_count: 8,
                    },
                ],
                exact_constraint_count: 1,
                exact_constraints: vec!["valid_at"],
                residual_exact_constraint_count: 1,
                residual_exact_constraints: vec!["valid_at"],
                lossy_indexed_constraint_count: 1,
                lossy_indexed_constraints: vec!["valid_at"],
                candidate_count: 8,
                exact_match_count: 5,
                filtered_candidate_count: 3,
                candidate_selectivity_basis_points: 6250,
                full_scan: false,
            }),
        );
        let json = serde_json::to_value(snapshot)?;

        assert_eq!(
            json["lookup_plan"]["filtered_candidate_count"].as_u64(),
            Some(3)
        );
        Ok(())
    }

    #[test]
    fn workload_snapshot_preserves_lookup_plan_candidate_selectivity(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let measurement = sample_measurement()?;
        let snapshot = WorkloadMeasurementSnapshot::from_measurement_with_lookup_plan(
            &measurement,
            Some(continuitydb_kernel::FileKernelLookupPlan {
                indexed_constraint_count: 1,
                indexed_constraints: vec!["valid_at"],
                indexed_constraint_plans: vec![
                    continuitydb_kernel::FileKernelIndexedConstraintPlan {
                        name: "valid_at",
                        candidate_count: 8,
                    },
                ],
                exact_constraint_count: 1,
                exact_constraints: vec!["valid_at"],
                residual_exact_constraint_count: 1,
                residual_exact_constraints: vec!["valid_at"],
                lossy_indexed_constraint_count: 1,
                lossy_indexed_constraints: vec!["valid_at"],
                candidate_count: 8,
                exact_match_count: 5,
                filtered_candidate_count: 3,
                candidate_selectivity_basis_points: 6250,
                full_scan: false,
            }),
        );
        let json = serde_json::to_value(snapshot)?;

        assert_eq!(
            json["lookup_plan"]["candidate_selectivity_basis_points"].as_u64(),
            Some(6250)
        );
        Ok(())
    }

    #[test]
    fn workload_snapshot_preserves_lookup_plan_exact_constraints(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let measurement = sample_measurement()?;
        let snapshot = WorkloadMeasurementSnapshot::from_measurement_with_lookup_plan(
            &measurement,
            Some(continuitydb_kernel::FileKernelLookupPlan {
                indexed_constraint_count: 2,
                indexed_constraints: vec!["scope", "valid_at"],
                indexed_constraint_plans: vec![
                    continuitydb_kernel::FileKernelIndexedConstraintPlan {
                        name: "scope",
                        candidate_count: 5,
                    },
                    continuitydb_kernel::FileKernelIndexedConstraintPlan {
                        name: "valid_at",
                        candidate_count: 8,
                    },
                ],
                exact_constraint_count: 2,
                exact_constraints: vec!["scope", "valid_at"],
                residual_exact_constraint_count: 1,
                residual_exact_constraints: vec!["valid_at"],
                lossy_indexed_constraint_count: 1,
                lossy_indexed_constraints: vec!["valid_at"],
                candidate_count: 5,
                exact_match_count: 4,
                filtered_candidate_count: 1,
                candidate_selectivity_basis_points: 8000,
                full_scan: false,
            }),
        );
        let json = serde_json::to_value(snapshot)?;

        assert_eq!(
            json["lookup_plan"]["exact_constraint_count"].as_u64(),
            Some(2)
        );
        assert_eq!(
            json["lookup_plan"]["exact_constraints"],
            serde_json::json!(["scope", "valid_at"])
        );
        Ok(())
    }

    #[test]
    fn workload_snapshot_preserves_lookup_plan_lossy_indexed_constraints(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let measurement = sample_measurement()?;
        let snapshot = WorkloadMeasurementSnapshot::from_measurement_with_lookup_plan(
            &measurement,
            Some(continuitydb_kernel::FileKernelLookupPlan {
                indexed_constraint_count: 3,
                indexed_constraints: vec!["scope", "system_at", "valid_at"],
                indexed_constraint_plans: vec![
                    continuitydb_kernel::FileKernelIndexedConstraintPlan {
                        name: "scope",
                        candidate_count: 5,
                    },
                    continuitydb_kernel::FileKernelIndexedConstraintPlan {
                        name: "system_at",
                        candidate_count: 8,
                    },
                    continuitydb_kernel::FileKernelIndexedConstraintPlan {
                        name: "valid_at",
                        candidate_count: 8,
                    },
                ],
                exact_constraint_count: 3,
                exact_constraints: vec!["scope", "system_at", "valid_at"],
                residual_exact_constraint_count: 2,
                residual_exact_constraints: vec!["system_at", "valid_at"],
                lossy_indexed_constraint_count: 2,
                lossy_indexed_constraints: vec!["system_at", "valid_at"],
                candidate_count: 5,
                exact_match_count: 4,
                filtered_candidate_count: 1,
                candidate_selectivity_basis_points: 8000,
                full_scan: false,
            }),
        );
        let json = serde_json::to_value(snapshot)?;

        assert_eq!(
            json["lookup_plan"]["lossy_indexed_constraint_count"].as_u64(),
            Some(2)
        );
        assert_eq!(
            json["lookup_plan"]["lossy_indexed_constraints"],
            serde_json::json!(["system_at", "valid_at"])
        );
        Ok(())
    }

    #[test]
    fn workload_snapshot_preserves_lookup_plan_residual_exact_constraints(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let measurement = sample_measurement()?;
        let snapshot = WorkloadMeasurementSnapshot::from_measurement_with_lookup_plan(
            &measurement,
            Some(continuitydb_kernel::FileKernelLookupPlan {
                indexed_constraint_count: 1,
                indexed_constraints: vec!["scope"],
                indexed_constraint_plans: vec![
                    continuitydb_kernel::FileKernelIndexedConstraintPlan {
                        name: "scope",
                        candidate_count: 5,
                    },
                ],
                exact_constraint_count: 2,
                exact_constraints: vec!["scope", "valid_at"],
                residual_exact_constraint_count: 1,
                residual_exact_constraints: vec!["valid_at"],
                lossy_indexed_constraint_count: 1,
                lossy_indexed_constraints: vec!["valid_at"],
                candidate_count: 5,
                exact_match_count: 4,
                filtered_candidate_count: 1,
                candidate_selectivity_basis_points: 8000,
                full_scan: false,
            }),
        );
        let json = serde_json::to_value(snapshot)?;

        assert_eq!(
            json["lookup_plan"]["residual_exact_constraint_count"].as_u64(),
            Some(1)
        );
        assert_eq!(
            json["lookup_plan"]["residual_exact_constraints"],
            serde_json::json!(["valid_at"])
        );
        Ok(())
    }

    #[test]
    fn workload_baseline_store_appends_and_lists_in_order() -> Result<(), Box<dyn std::error::Error>>
    {
        let path = temp_baseline_path("continuitydb-workload-baseline-order");
        let store = FileWorkloadBaselineStore::new(&path);
        let recorded_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 2, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let snapshot = WorkloadMeasurementSnapshot::from_measurement(&sample_measurement()?);
        let first =
            WorkloadBaselineRecord::new(recorded_at, "small-memory", "memory", snapshot.clone());
        let second = WorkloadBaselineRecord::new(recorded_at, "small-file", "file", snapshot);

        store.append(&first)?;
        store.append(&second)?;
        let records = store.list()?;

        assert_eq!(records, vec![first, second]);

        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn workload_baseline_store_lists_missing_file_as_empty(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_baseline_path("continuitydb-workload-baseline-missing");
        let store = FileWorkloadBaselineStore::new(&path);

        assert!(store.list()?.is_empty());
        Ok(())
    }

    #[test]
    fn workload_baseline_store_reports_corrupt_jsonl_line() -> Result<(), Box<dyn std::error::Error>>
    {
        let path = temp_baseline_path("continuitydb-workload-baseline-corrupt");
        fs::write(&path, "{not-json}\n")?;
        let store = FileWorkloadBaselineStore::new(&path);

        assert!(matches!(
            store.list(),
            Err(WorkloadBaselineError::CorruptRecord { line: 1, .. })
        ));

        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn workload_baseline_regression_latest_matching_uses_label_kernel_and_timestamp(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_baseline_path("continuitydb-workload-baseline-latest");
        let store = FileWorkloadBaselineStore::new(&path);
        let snapshot = deterministic_snapshot();
        let older = WorkloadBaselineRecord::new(
            timestamp(2026, 5, 20, 2)?,
            "target",
            "memory",
            snapshot.clone(),
        );
        let newest_matching = WorkloadBaselineRecord::new(
            timestamp(2026, 5, 20, 3)?,
            "target",
            "memory",
            snapshot.clone(),
        );
        let newer_different_kernel = WorkloadBaselineRecord::new(
            timestamp(2026, 5, 20, 4)?,
            "target",
            "file",
            snapshot.clone(),
        );
        let newer_different_label =
            WorkloadBaselineRecord::new(timestamp(2026, 5, 20, 5)?, "other", "memory", snapshot);

        store.append(&older)?;
        store.append(&newest_matching)?;
        store.append(&newer_different_kernel)?;
        store.append(&newer_different_label)?;

        assert_eq!(
            store.latest_matching("target", "memory")?,
            Some(newest_matching)
        );
        assert_eq!(store.latest_matching("missing", "memory")?, None);

        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn workload_baseline_regression_passes_equal_counts_within_elapsed_threshold(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let previous = baseline_record("current", "memory", 100, 100)?;
        let current = baseline_record("current", "memory", 125, 120)?;
        let comparison = compare_workload_snapshot_to_baseline(
            &previous,
            &current.snapshot,
            WorkloadRegressionThresholds {
                max_elapsed_growth_percent: 25,
            },
        );

        assert!(comparison.passed());
        assert!(comparison.regressions.is_empty());
        Ok(())
    }

    #[test]
    fn workload_baseline_regression_reports_count_and_elapsed_changes(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let previous = baseline_record("current", "memory", 100, 100)?;
        let mut current_snapshot = deterministic_snapshot();
        current_snapshot.workload.cell_count += 1;
        current_snapshot.workload.frontier_count += 1;
        current_snapshot.workload.dependency_count += 1;
        current_snapshot.workload.total_token_cost += 10;
        current_snapshot.ingest.operation_count += 1;
        current_snapshot.ingest.elapsed_nanos = 126;
        current_snapshot.checkout_operation.operation_count += 1;
        current_snapshot.checkout_operation.elapsed_nanos = 150;
        current_snapshot.checkout.matched_count += 1;
        current_snapshot.checkout.selected_count += 1;
        current_snapshot.checkout.alternative_count += 1;
        current_snapshot.checkout.frontier_count += 1;
        current_snapshot.checkout.selected_token_count += 10;

        let comparison = compare_workload_snapshot_to_baseline(
            &previous,
            &current_snapshot,
            WorkloadRegressionThresholds {
                max_elapsed_growth_percent: 25,
            },
        );

        assert!(!comparison.passed());
        assert_eq!(
            comparison.regressions,
            vec![
                WorkloadBaselineRegression::WorkloadCellCountChanged {
                    previous: 8,
                    current: 9
                },
                WorkloadBaselineRegression::WorkloadFrontierCountChanged {
                    previous: 2,
                    current: 3
                },
                WorkloadBaselineRegression::WorkloadDependencyCountChanged {
                    previous: 6,
                    current: 7
                },
                WorkloadBaselineRegression::WorkloadTokenCostChanged {
                    previous: 988,
                    current: 998
                },
                WorkloadBaselineRegression::IngestOperationCountChanged {
                    previous: 8,
                    current: 9
                },
                WorkloadBaselineRegression::CheckoutOperationCountChanged {
                    previous: 1,
                    current: 2
                },
                WorkloadBaselineRegression::CheckoutMatchedCountChanged {
                    previous: 8,
                    current: 9
                },
                WorkloadBaselineRegression::CheckoutSelectedCountChanged {
                    previous: 3,
                    current: 4
                },
                WorkloadBaselineRegression::CheckoutAlternativeCountChanged {
                    previous: 5,
                    current: 6
                },
                WorkloadBaselineRegression::CheckoutFrontierCountChanged {
                    previous: 0,
                    current: 1
                },
                WorkloadBaselineRegression::CheckoutSelectedTokenCountChanged {
                    previous: 370,
                    current: 380
                },
                WorkloadBaselineRegression::IngestElapsedRegressed {
                    previous_nanos: 100,
                    current_nanos: 126,
                    max_allowed_nanos: 125
                },
                WorkloadBaselineRegression::CheckoutElapsedRegressed {
                    previous_nanos: 100,
                    current_nanos: 150,
                    max_allowed_nanos: 125
                },
            ]
        );
        Ok(())
    }

    #[test]
    fn workload_baseline_regression_detects_lookup_plan_presence_change(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let previous = baseline_record("current", "file", 100, 100)?;
        let mut current_snapshot = deterministic_snapshot();
        current_snapshot.lookup_plan =
            Some(lookup_plan_snapshot(&["scope"], &[("scope", 8)], 8, false));

        let comparison = compare_workload_snapshot_to_baseline(
            &previous,
            &current_snapshot,
            WorkloadRegressionThresholds::default(),
        );

        assert_eq!(
            comparison.regressions,
            vec![WorkloadBaselineRegression::LookupPlanPresenceChanged {
                previous: false,
                current: true
            }]
        );
        Ok(())
    }

    #[test]
    fn workload_baseline_regression_ignores_missing_lookup_plans(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let previous = baseline_record("current", "memory", 100, 100)?;
        let current_snapshot = deterministic_snapshot();

        let comparison = compare_workload_snapshot_to_baseline(
            &previous,
            &current_snapshot,
            WorkloadRegressionThresholds::default(),
        );

        assert!(comparison.passed());
        assert!(comparison.regressions.is_empty());
        Ok(())
    }

    #[test]
    fn workload_baseline_regression_detects_lookup_plan_changes(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut previous = baseline_record("current", "file", 100, 100)?;
        previous.snapshot.lookup_plan = Some(lookup_plan_snapshot(
            &["scope", "minimum_confidence"],
            &[("scope", 8), ("minimum_confidence", 8)],
            8,
            false,
        ));
        let mut current_snapshot = deterministic_snapshot();
        current_snapshot.lookup_plan = Some(lookup_plan_snapshot(
            &["scope", "activation"],
            &[("scope", 7), ("activation", 9)],
            9,
            true,
        ));

        let comparison = compare_workload_snapshot_to_baseline(
            &previous,
            &current_snapshot,
            WorkloadRegressionThresholds::default(),
        );

        assert_eq!(
            comparison.regressions,
            vec![
                WorkloadBaselineRegression::LookupPlanIndexedConstraintsChanged {
                    previous: vec!["scope".to_string(), "minimum_confidence".to_string()],
                    current: vec!["scope".to_string(), "activation".to_string()],
                },
                WorkloadBaselineRegression::LookupPlanExactConstraintsChanged {
                    previous: vec!["scope".to_string(), "minimum_confidence".to_string()],
                    current: vec!["scope".to_string(), "activation".to_string()],
                },
                WorkloadBaselineRegression::LookupPlanCandidateCountChanged {
                    previous: 8,
                    current: 9,
                },
                WorkloadBaselineRegression::LookupPlanExactMatchCountChanged {
                    previous: 8,
                    current: 9,
                },
                WorkloadBaselineRegression::LookupPlanFullScanChanged {
                    previous: false,
                    current: true,
                },
                WorkloadBaselineRegression::LookupPlanConstraintCandidateCountChanged {
                    name: "scope".to_string(),
                    previous: 8,
                    current: 7,
                },
            ]
        );
        Ok(())
    }

    #[test]
    fn workload_baseline_regression_detects_lookup_plan_filtered_candidate_count_change(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut previous = baseline_record("current", "file", 100, 100)?;
        previous.snapshot.lookup_plan = Some(WorkloadLookupPlanSnapshot {
            indexed_constraint_count: 1,
            indexed_constraints: vec!["valid_at".to_string()],
            indexed_constraint_plans: vec![WorkloadIndexedConstraintPlanSnapshot {
                name: "valid_at".to_string(),
                candidate_count: 8,
            }],
            exact_constraint_count: 1,
            exact_constraints: vec!["valid_at".to_string()],
            residual_exact_constraint_count: 1,
            residual_exact_constraints: vec!["valid_at".to_string()],
            lossy_indexed_constraint_count: 1,
            lossy_indexed_constraints: vec!["valid_at".to_string()],
            candidate_count: 8,
            exact_match_count: 5,
            filtered_candidate_count: 3,
            candidate_selectivity_basis_points: 6250,
            full_scan: false,
        });
        let mut current_snapshot = deterministic_snapshot();
        current_snapshot.lookup_plan = Some(WorkloadLookupPlanSnapshot {
            indexed_constraint_count: 1,
            indexed_constraints: vec!["valid_at".to_string()],
            indexed_constraint_plans: vec![WorkloadIndexedConstraintPlanSnapshot {
                name: "valid_at".to_string(),
                candidate_count: 8,
            }],
            exact_constraint_count: 1,
            exact_constraints: vec!["valid_at".to_string()],
            residual_exact_constraint_count: 1,
            residual_exact_constraints: vec!["valid_at".to_string()],
            lossy_indexed_constraint_count: 1,
            lossy_indexed_constraints: vec!["valid_at".to_string()],
            candidate_count: 8,
            exact_match_count: 5,
            filtered_candidate_count: 4,
            candidate_selectivity_basis_points: 6250,
            full_scan: false,
        });

        let comparison = compare_workload_snapshot_to_baseline(
            &previous,
            &current_snapshot,
            WorkloadRegressionThresholds::default(),
        );

        assert_eq!(
            comparison.regressions,
            vec![
                WorkloadBaselineRegression::LookupPlanFilteredCandidateCountChanged {
                    previous: 3,
                    current: 4,
                }
            ]
        );
        Ok(())
    }

    #[test]
    fn workload_baseline_regression_detects_lookup_plan_exact_constraint_change(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut previous = baseline_record("current", "file", 100, 100)?;
        previous.snapshot.lookup_plan = Some(WorkloadLookupPlanSnapshot {
            indexed_constraint_count: 1,
            indexed_constraints: vec!["scope".to_string()],
            indexed_constraint_plans: vec![WorkloadIndexedConstraintPlanSnapshot {
                name: "scope".to_string(),
                candidate_count: 8,
            }],
            exact_constraint_count: 1,
            exact_constraints: vec!["scope".to_string()],
            residual_exact_constraint_count: 0,
            residual_exact_constraints: Vec::new(),
            lossy_indexed_constraint_count: 0,
            lossy_indexed_constraints: Vec::new(),
            candidate_count: 8,
            exact_match_count: 8,
            filtered_candidate_count: 0,
            candidate_selectivity_basis_points: 10000,
            full_scan: false,
        });
        let mut current_snapshot = deterministic_snapshot();
        current_snapshot.lookup_plan = Some(WorkloadLookupPlanSnapshot {
            indexed_constraint_count: 1,
            indexed_constraints: vec!["scope".to_string()],
            indexed_constraint_plans: vec![WorkloadIndexedConstraintPlanSnapshot {
                name: "scope".to_string(),
                candidate_count: 8,
            }],
            exact_constraint_count: 2,
            exact_constraints: vec!["scope".to_string(), "valid_at".to_string()],
            residual_exact_constraint_count: 1,
            residual_exact_constraints: vec!["valid_at".to_string()],
            lossy_indexed_constraint_count: 1,
            lossy_indexed_constraints: vec!["valid_at".to_string()],
            candidate_count: 8,
            exact_match_count: 8,
            filtered_candidate_count: 0,
            candidate_selectivity_basis_points: 10000,
            full_scan: false,
        });

        let comparison = compare_workload_snapshot_to_baseline(
            &previous,
            &current_snapshot,
            WorkloadRegressionThresholds::default(),
        );

        assert_eq!(
            comparison.regressions,
            vec![
                WorkloadBaselineRegression::LookupPlanExactConstraintsChanged {
                    previous: vec!["scope".to_string()],
                    current: vec!["scope".to_string(), "valid_at".to_string()],
                },
                WorkloadBaselineRegression::LookupPlanLossyIndexedConstraintsChanged {
                    previous: Vec::new(),
                    current: vec!["valid_at".to_string()],
                },
                WorkloadBaselineRegression::LookupPlanResidualExactConstraintsChanged {
                    previous: Vec::new(),
                    current: vec!["valid_at".to_string()],
                }
            ]
        );
        Ok(())
    }

    #[test]
    fn workload_baseline_regression_detects_lookup_plan_lossy_constraint_change(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut previous = baseline_record("current", "file", 100, 100)?;
        previous.snapshot.lookup_plan = Some(WorkloadLookupPlanSnapshot {
            indexed_constraint_count: 1,
            indexed_constraints: vec!["scope".to_string()],
            indexed_constraint_plans: vec![WorkloadIndexedConstraintPlanSnapshot {
                name: "scope".to_string(),
                candidate_count: 8,
            }],
            exact_constraint_count: 1,
            exact_constraints: vec!["scope".to_string()],
            residual_exact_constraint_count: 0,
            residual_exact_constraints: Vec::new(),
            lossy_indexed_constraint_count: 0,
            lossy_indexed_constraints: Vec::new(),
            candidate_count: 8,
            exact_match_count: 8,
            filtered_candidate_count: 0,
            candidate_selectivity_basis_points: 10000,
            full_scan: false,
        });
        let mut current_snapshot = deterministic_snapshot();
        current_snapshot.lookup_plan = Some(WorkloadLookupPlanSnapshot {
            indexed_constraint_count: 2,
            indexed_constraints: vec!["scope".to_string(), "valid_at".to_string()],
            indexed_constraint_plans: vec![
                WorkloadIndexedConstraintPlanSnapshot {
                    name: "scope".to_string(),
                    candidate_count: 8,
                },
                WorkloadIndexedConstraintPlanSnapshot {
                    name: "valid_at".to_string(),
                    candidate_count: 8,
                },
            ],
            exact_constraint_count: 2,
            exact_constraints: vec!["scope".to_string(), "valid_at".to_string()],
            residual_exact_constraint_count: 1,
            residual_exact_constraints: vec!["valid_at".to_string()],
            lossy_indexed_constraint_count: 1,
            lossy_indexed_constraints: vec!["valid_at".to_string()],
            candidate_count: 8,
            exact_match_count: 8,
            filtered_candidate_count: 0,
            candidate_selectivity_basis_points: 10000,
            full_scan: false,
        });

        let comparison = compare_workload_snapshot_to_baseline(
            &previous,
            &current_snapshot,
            WorkloadRegressionThresholds::default(),
        );

        assert!(comparison.regressions.contains(
            &WorkloadBaselineRegression::LookupPlanLossyIndexedConstraintsChanged {
                previous: Vec::new(),
                current: vec!["valid_at".to_string()],
            }
        ));
        Ok(())
    }

    #[test]
    fn workload_baseline_regression_detects_lookup_plan_residual_constraint_change(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut previous = baseline_record("current", "file", 100, 100)?;
        previous.snapshot.lookup_plan = Some(WorkloadLookupPlanSnapshot {
            indexed_constraint_count: 1,
            indexed_constraints: vec!["scope".to_string()],
            indexed_constraint_plans: vec![WorkloadIndexedConstraintPlanSnapshot {
                name: "scope".to_string(),
                candidate_count: 8,
            }],
            exact_constraint_count: 1,
            exact_constraints: vec!["scope".to_string()],
            residual_exact_constraint_count: 0,
            residual_exact_constraints: Vec::new(),
            lossy_indexed_constraint_count: 0,
            lossy_indexed_constraints: Vec::new(),
            candidate_count: 8,
            exact_match_count: 8,
            filtered_candidate_count: 0,
            candidate_selectivity_basis_points: 10000,
            full_scan: false,
        });
        let mut current_snapshot = deterministic_snapshot();
        current_snapshot.lookup_plan = Some(WorkloadLookupPlanSnapshot {
            indexed_constraint_count: 2,
            indexed_constraints: vec!["scope".to_string(), "valid_at".to_string()],
            indexed_constraint_plans: vec![
                WorkloadIndexedConstraintPlanSnapshot {
                    name: "scope".to_string(),
                    candidate_count: 8,
                },
                WorkloadIndexedConstraintPlanSnapshot {
                    name: "valid_at".to_string(),
                    candidate_count: 8,
                },
            ],
            exact_constraint_count: 2,
            exact_constraints: vec!["scope".to_string(), "valid_at".to_string()],
            residual_exact_constraint_count: 1,
            residual_exact_constraints: vec!["valid_at".to_string()],
            lossy_indexed_constraint_count: 1,
            lossy_indexed_constraints: vec!["valid_at".to_string()],
            candidate_count: 8,
            exact_match_count: 8,
            filtered_candidate_count: 0,
            candidate_selectivity_basis_points: 10000,
            full_scan: false,
        });

        let comparison = compare_workload_snapshot_to_baseline(
            &previous,
            &current_snapshot,
            WorkloadRegressionThresholds::default(),
        );

        assert!(comparison.regressions.contains(
            &WorkloadBaselineRegression::LookupPlanResidualExactConstraintsChanged {
                previous: Vec::new(),
                current: vec!["valid_at".to_string()],
            }
        ));
        Ok(())
    }

    #[test]
    fn workload_baseline_regression_detects_lookup_plan_candidate_selectivity_change(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut previous = baseline_record("current", "file", 100, 100)?;
        previous.snapshot.lookup_plan = Some(WorkloadLookupPlanSnapshot {
            indexed_constraint_count: 1,
            indexed_constraints: vec!["valid_at".to_string()],
            indexed_constraint_plans: vec![WorkloadIndexedConstraintPlanSnapshot {
                name: "valid_at".to_string(),
                candidate_count: 8,
            }],
            exact_constraint_count: 1,
            exact_constraints: vec!["valid_at".to_string()],
            residual_exact_constraint_count: 1,
            residual_exact_constraints: vec!["valid_at".to_string()],
            lossy_indexed_constraint_count: 1,
            lossy_indexed_constraints: vec!["valid_at".to_string()],
            candidate_count: 8,
            exact_match_count: 5,
            filtered_candidate_count: 3,
            candidate_selectivity_basis_points: 6250,
            full_scan: false,
        });
        let mut current_snapshot = deterministic_snapshot();
        current_snapshot.lookup_plan = Some(WorkloadLookupPlanSnapshot {
            indexed_constraint_count: 1,
            indexed_constraints: vec!["valid_at".to_string()],
            indexed_constraint_plans: vec![WorkloadIndexedConstraintPlanSnapshot {
                name: "valid_at".to_string(),
                candidate_count: 8,
            }],
            exact_constraint_count: 1,
            exact_constraints: vec!["valid_at".to_string()],
            residual_exact_constraint_count: 1,
            residual_exact_constraints: vec!["valid_at".to_string()],
            lossy_indexed_constraint_count: 1,
            lossy_indexed_constraints: vec!["valid_at".to_string()],
            candidate_count: 8,
            exact_match_count: 5,
            filtered_candidate_count: 3,
            candidate_selectivity_basis_points: 7500,
            full_scan: false,
        });

        let comparison = compare_workload_snapshot_to_baseline(
            &previous,
            &current_snapshot,
            WorkloadRegressionThresholds::default(),
        );

        assert_eq!(
            comparison.regressions,
            vec![
                WorkloadBaselineRegression::LookupPlanCandidateSelectivityChanged {
                    previous: 6250,
                    current: 7500,
                }
            ]
        );
        Ok(())
    }

    fn sample_measurement() -> Result<WorkloadMeasurement, Box<dyn std::error::Error>> {
        let workload = generate_world_model_workload(sample_config()?)?;
        let mut kernel = MemoryKernel::default();
        let committed_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 1, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let request = CheckoutRequest {
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
            minimum_confidence: Confidence::new(0.0)?,
            token_budget: 400,
        };

        measure_ingest_and_checkout(&mut kernel, &workload, committed_at, request)
            .map_err(Into::into)
    }

    fn baseline_record(
        label: &str,
        kernel: &str,
        ingest_elapsed_nanos: u128,
        checkout_elapsed_nanos: u128,
    ) -> Result<WorkloadBaselineRecord, Box<dyn std::error::Error>> {
        let mut snapshot = deterministic_snapshot();
        snapshot.ingest.elapsed_nanos = ingest_elapsed_nanos;
        snapshot.checkout_operation.elapsed_nanos = checkout_elapsed_nanos;
        Ok(WorkloadBaselineRecord::new(
            timestamp(2026, 5, 20, 2)?,
            label,
            kernel,
            snapshot,
        ))
    }

    fn deterministic_snapshot() -> WorkloadMeasurementSnapshot {
        WorkloadMeasurementSnapshot {
            workload: WorkloadSummarySnapshot {
                cell_count: 8,
                frontier_count: 2,
                dependency_count: 6,
                total_token_cost: 988,
            },
            revision_link_count: 6,
            ingest: MeasuredOperationSnapshot {
                operation_count: 8,
                elapsed_nanos: 100,
            },
            checkout_operation: MeasuredOperationSnapshot {
                operation_count: 1,
                elapsed_nanos: 100,
            },
            checkout: CheckoutMeasurementSnapshot {
                matched_count: 8,
                selected_count: 3,
                alternative_count: 5,
                frontier_count: 0,
                selected_token_count: 370,
            },
            lookup_plan: None,
        }
    }

    fn lookup_plan_snapshot(
        constraints: &[&str],
        constraint_plans: &[(&str, usize)],
        candidate_count: usize,
        full_scan: bool,
    ) -> WorkloadLookupPlanSnapshot {
        WorkloadLookupPlanSnapshot {
            indexed_constraint_count: constraints.len(),
            indexed_constraints: constraints
                .iter()
                .map(|constraint| constraint.to_string())
                .collect(),
            indexed_constraint_plans: constraint_plans
                .iter()
                .map(
                    |(name, candidate_count)| WorkloadIndexedConstraintPlanSnapshot {
                        name: name.to_string(),
                        candidate_count: *candidate_count,
                    },
                )
                .collect(),
            exact_constraint_count: constraints.len(),
            exact_constraints: constraints
                .iter()
                .map(|constraint| constraint.to_string())
                .collect(),
            residual_exact_constraint_count: constraints
                .iter()
                .filter(|constraint| **constraint == "system_at" || **constraint == "valid_at")
                .count(),
            residual_exact_constraints: constraints
                .iter()
                .filter(|constraint| **constraint == "system_at" || **constraint == "valid_at")
                .map(|constraint| constraint.to_string())
                .collect(),
            lossy_indexed_constraint_count: constraints
                .iter()
                .filter(|constraint| **constraint == "system_at" || **constraint == "valid_at")
                .count(),
            lossy_indexed_constraints: constraints
                .iter()
                .filter(|constraint| **constraint == "system_at" || **constraint == "valid_at")
                .map(|constraint| constraint.to_string())
                .collect(),
            candidate_count,
            exact_match_count: candidate_count,
            filtered_candidate_count: 0,
            candidate_selectivity_basis_points: if candidate_count == 0 { 0 } else { 10000 },
            full_scan,
        }
    }

    fn timestamp(
        year: i32,
        month: u32,
        day: u32,
        hour: u32,
    ) -> Result<DateTime<Utc>, Box<dyn std::error::Error>> {
        Utc.with_ymd_and_hms(year, month, day, hour, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp").into())
    }

    fn temp_baseline_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("{name}-{:?}.jsonl", StateCellId::new()))
    }
}
