//! Deterministic workload generation for ContinuityDB benchmarks.

use chrono::{DateTime, Utc};
use continuitydb_checkout::{checkout, CheckoutError, CheckoutRequest};
use continuitydb_core::{
    ActivationState, Answerability, CellCost, CellDependency, CellDependencyKind, CellPayload,
    Citation, Confidence, Evidence, Scope, SemanticAnchor, StateCell, StateCellId, TrustSignal,
    UtilityFeedback, ValidTimeRange,
};
use continuitydb_kernel::{KernelError, StorageKernel};
use std::time::{Duration, Instant};
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
    /// Ingest operation measurement.
    pub ingest: MeasuredOperation,
    /// Checkout operation measurement.
    pub checkout_operation: MeasuredOperation,
    /// Checkout result counts.
    pub checkout: CheckoutMeasurement,
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
    let ingest_elapsed = ingest_started.elapsed();

    let checkout_started = Instant::now();
    let slice = checkout(kernel, request)?;
    let checkout_elapsed = checkout_started.elapsed();

    Ok(WorkloadMeasurement {
        workload_summary: workload.summary,
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
    use continuitydb_core::{ActivationState, CellDependencyKind, Confidence, Scope};
    use continuitydb_memory::MemoryKernel;

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
            answerability_question: None,
            evidence_source: None,
            dependency_target: None,
            dependency_kind: None,
            minimum_confidence: Confidence::new(0.0)?,
            token_budget: 400,
        };

        let measurement =
            measure_ingest_and_checkout(&mut kernel, &workload, committed_at, request)?;

        assert_eq!(measurement.workload_summary, workload.summary);
        assert_eq!(measurement.ingest.operation_count, 8);
        assert_eq!(measurement.checkout.matched_count, 8);
        assert_eq!(measurement.checkout.selected_count, 3);
        assert_eq!(measurement.checkout.alternative_count, 5);
        assert_eq!(measurement.checkout.frontier_count, 0);
        assert!(measurement.checkout.selected_token_count <= 400);
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
            answerability_question: None,
            evidence_source: None,
            dependency_target: None,
            dependency_kind: None,
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
            answerability_question: None,
            evidence_source: None,
            dependency_target: None,
            dependency_kind: None,
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
}
