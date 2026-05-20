//! In-memory StorageKernel implementation for correctness tests.

use continuitydb_core::StateCell;
use continuitydb_kernel::{CellLookup, KernelError, StorageKernel};

/// Append-only in-memory storage kernel.
#[derive(Default)]
pub struct MemoryKernel {
    cells: Vec<StateCell>,
}

impl StorageKernel for MemoryKernel {
    fn append_cell(&mut self, cell: StateCell) -> Result<(), KernelError> {
        if self.cells.iter().any(|stored| stored.id == cell.id) {
            return Err(KernelError::DuplicateCell);
        }

        self.cells.push(cell);
        Ok(())
    }

    fn lookup_cells(&self, lookup: CellLookup) -> Result<Vec<StateCell>, KernelError> {
        let cells = self
            .cells
            .iter()
            .filter(|cell| {
                lookup.semantic_anchor.as_ref().map_or(true, |anchor| {
                    cell.anchors
                        .iter()
                        .any(|candidate| candidate.as_str() == anchor)
                })
            })
            .filter(|cell| {
                lookup
                    .scope
                    .as_ref()
                    .map_or(true, |scope| &cell.scope == scope)
            })
            .filter(|cell| {
                lookup
                    .valid_at
                    .map_or(true, |valid_at| cell.valid_time.contains(valid_at))
            })
            .filter(|cell| {
                lookup
                    .activation
                    .map_or(true, |activation| cell.activation == activation)
            })
            .filter(|cell| {
                lookup
                    .answerability_question
                    .as_ref()
                    .map_or(true, |question| {
                        cell.answerability
                            .questions()
                            .iter()
                            .any(|candidate| candidate == question)
                    })
            })
            .filter(|cell| {
                lookup.evidence_source.as_ref().map_or(true, |source| {
                    cell.evidence
                        .iter()
                        .any(|evidence| evidence.source.as_str() == source)
                })
            })
            .filter(|cell| {
                lookup
                    .minimum_confidence
                    .map_or(true, |minimum_confidence| {
                        cell.evidence.iter().any(|evidence| {
                            evidence.confidence.value() >= minimum_confidence.value()
                        })
                    })
            })
            .filter(|cell| {
                lookup.dependency_target.map_or(true, |target| {
                    cell.dependencies.iter().any(|dependency| {
                        dependency.target == target
                            && lookup
                                .dependency_kind
                                .map_or(true, |kind| dependency.kind == kind)
                    })
                })
            })
            .cloned()
            .collect();

        Ok(cells)
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use continuitydb_core::{
        ActivationState, Answerability, CellCost, CellDependency, CellDependencyKind, CellPayload,
        Citation, Confidence, Evidence, Scope, SemanticAnchor, SourceId, StateCell, StateCellId,
        TrustSignal, ValidTimeRange,
    };
    use continuitydb_kernel::{CellLookup, StorageKernel};

    use super::MemoryKernel;

    fn sample_cell(
        anchor: &str,
        confidence: f32,
        tokens: i64,
    ) -> Result<StateCell, Box<dyn std::error::Error>> {
        sample_cell_with_source(anchor, "test", confidence, tokens)
    }

    fn sample_cell_with_source(
        anchor: &str,
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
            Answerability::new(vec!["what is stored?".to_string()])?,
            vec![Evidence {
                source: SourceId::new(source),
                citation: Citation {
                    locator: "test://sample".to_string(),
                },
                confidence: Confidence::new(confidence)?,
                trust: vec![TrustSignal::DirectObservation],
            }],
            CellPayload::Text(anchor.to_string()),
            CellCost::new(tokens, 0)?,
        )
        .map_err(Into::into)
    }

    #[test]
    fn append_and_lookup_by_anchor_preserves_cells() -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let cell = sample_cell("project:continuitydb:status", 0.9, 12)?;

        kernel.append_cell(cell.clone())?;
        let results = kernel.lookup_cells(CellLookup {
            semantic_anchor: Some("project:continuitydb:status".to_string()),
            ..CellLookup::default()
        })?;

        assert_eq!(results, vec![cell]);
        Ok(())
    }

    #[test]
    fn memory_kernel_filters_by_activation_state() -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let mut active = sample_cell("project:continuitydb:active", 0.9, 12)?;
        active.activation = ActivationState::Active;
        let mut frontier = sample_cell("project:continuitydb:frontier", 0.8, 15)?;
        frontier.activation = ActivationState::Frontier;
        kernel.append_cell(active)?;
        kernel.append_cell(frontier.clone())?;

        let results = kernel.lookup_cells(CellLookup {
            activation: Some(ActivationState::Frontier),
            ..CellLookup::default()
        })?;

        assert_eq!(results, vec![frontier]);
        Ok(())
    }

    #[test]
    fn memory_kernel_filters_by_answerability_question() -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let mut status = sample_cell("project:continuitydb:status", 0.9, 12)?;
        status.answerability = Answerability::new(vec!["what is status?".to_string()])?;
        let mut frontier = sample_cell("project:continuitydb:frontier", 0.8, 15)?;
        frontier.answerability = Answerability::new(vec!["what is frontier?".to_string()])?;
        kernel.append_cell(status)?;
        kernel.append_cell(frontier.clone())?;

        let results = kernel.lookup_cells(CellLookup {
            answerability_question: Some("what is frontier?".to_string()),
            ..CellLookup::default()
        })?;

        assert_eq!(results, vec![frontier]);
        Ok(())
    }

    #[test]
    fn memory_kernel_filters_by_evidence_source() -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let observed = sample_cell_with_source("project:continuitydb:observed", "sensor", 0.9, 12)?;
        let reviewed = sample_cell_with_source("project:continuitydb:reviewed", "human", 0.8, 15)?;
        kernel.append_cell(observed)?;
        kernel.append_cell(reviewed.clone())?;

        let results = kernel.lookup_cells(CellLookup {
            evidence_source: Some("human".to_string()),
            ..CellLookup::default()
        })?;

        assert_eq!(results, vec![reviewed]);
        Ok(())
    }

    #[test]
    fn memory_kernel_filters_by_minimum_confidence() -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let weak = sample_cell("project:continuitydb:weak", 0.61, 12)?;
        let strong = sample_cell("project:continuitydb:strong", 0.86, 15)?;
        kernel.append_cell(weak)?;
        kernel.append_cell(strong.clone())?;

        let results = kernel.lookup_cells(CellLookup {
            minimum_confidence: Some(Confidence::new(0.8)?),
            ..CellLookup::default()
        })?;

        assert_eq!(results, vec![strong]);
        Ok(())
    }

    #[test]
    fn memory_kernel_filters_by_dependency_target_and_kind(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let target = StateCellId::new();
        let other_target = StateCellId::new();
        let mut dependent = sample_cell("project:continuitydb:dependent", 0.9, 12)?;
        dependent.dependencies.push(CellDependency::new(
            target,
            CellDependencyKind::DependsOn,
            "depends on target",
        ));
        let mut unrelated = sample_cell("project:continuitydb:unrelated", 0.9, 12)?;
        unrelated.dependencies.push(CellDependency::new(
            other_target,
            CellDependencyKind::DependsOn,
            "depends on different target",
        ));
        let mut support = sample_cell("project:continuitydb:support", 0.9, 12)?;
        support.dependencies.push(CellDependency::new(
            target,
            CellDependencyKind::Supports,
            "supports target",
        ));
        kernel.append_cell(dependent.clone())?;
        kernel.append_cell(unrelated)?;
        kernel.append_cell(support)?;

        let results = kernel.lookup_cells(CellLookup {
            dependency_target: Some(target),
            dependency_kind: Some(CellDependencyKind::DependsOn),
            ..CellLookup::default()
        })?;

        assert_eq!(results, vec![dependent]);
        Ok(())
    }
}
