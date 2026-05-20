//! Deterministic workload generation for ContinuityDB benchmarks.

use chrono::{DateTime, Utc};
use continuitydb_core::{
    ActivationState, Answerability, CellCost, CellDependency, CellDependencyKind, CellPayload,
    Citation, Confidence, Evidence, Scope, SemanticAnchor, StateCell, StateCellId, TrustSignal,
    UtilityFeedback, ValidTimeRange,
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
    use continuitydb_core::{ActivationState, CellDependencyKind, Scope};

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
}
