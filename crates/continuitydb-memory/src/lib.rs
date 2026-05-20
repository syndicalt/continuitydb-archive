//! In-memory StorageKernel implementation for correctness tests.

use chrono::{DateTime, Utc};
use continuitydb_core::{CommitId, CommitManifest, StateCell, SystemTimeRange};
use continuitydb_kernel::{CellLookup, KernelError, StorageKernel};
use std::collections::{HashMap, HashSet};

/// Append-only in-memory storage kernel.
#[derive(Default)]
pub struct MemoryKernel {
    cells: Vec<StateCell>,
    manifests: HashMap<CommitId, CommitManifest>,
    manifest_order: Vec<CommitId>,
}

impl StorageKernel for MemoryKernel {
    fn append_cells_at<I>(
        &mut self,
        cells: I,
        committed_at: DateTime<Utc>,
    ) -> Result<(), KernelError>
    where
        I: IntoIterator<Item = StateCell>,
    {
        self.append_cells_at_with_commit_id(cells, committed_at, CommitId::new())
    }

    fn append_cells_at_with_commit_id<I>(
        &mut self,
        cells: I,
        committed_at: DateTime<Utc>,
        commit_id: CommitId,
    ) -> Result<(), KernelError>
    where
        I: IntoIterator<Item = StateCell>,
    {
        let mut batch_ids = HashSet::new();
        let mut stamped = Vec::new();
        for mut cell in cells {
            if self.cells.iter().any(|stored| stored.id == cell.id) || !batch_ids.insert(cell.id) {
                return Err(KernelError::DuplicateCell);
            }

            cell.system_time = SystemTimeRange::open_from(committed_at);
            cell.commit_id = commit_id;
            stamped.push(cell);
        }

        if stamped.is_empty() {
            return Ok(());
        }

        if self.manifests.contains_key(&commit_id) {
            return Err(KernelError::DuplicateCommit);
        }

        let cell_ids = stamped.iter().map(|cell| cell.id).collect::<Vec<_>>();
        self.cells.extend(stamped);
        self.manifests.insert(
            commit_id,
            CommitManifest::new(commit_id, committed_at, cell_ids),
        );
        self.manifest_order.push(commit_id);
        Ok(())
    }

    fn lookup_cells(&self, lookup: CellLookup) -> Result<Vec<StateCell>, KernelError> {
        let cells = self
            .cells
            .iter()
            .filter(|cell| lookup.cell_id.map_or(true, |cell_id| cell.id == cell_id))
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
                    .system_at
                    .map_or(true, |system_at| cell.system_time.contains(system_at))
            })
            .filter(|cell| {
                lookup
                    .commit_id
                    .map_or(true, |commit_id| cell.commit_id == commit_id)
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

    fn lookup_commit_manifest(
        &self,
        commit_id: CommitId,
    ) -> Result<Option<CommitManifest>, KernelError> {
        Ok(self.manifests.get(&commit_id).cloned())
    }

    fn list_commit_manifests(&self) -> Result<Vec<CommitManifest>, KernelError> {
        Ok(self
            .manifest_order
            .iter()
            .filter_map(|commit_id| self.manifests.get(commit_id).cloned())
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use continuitydb_core::{
        ActivationState, Answerability, CellCost, CellDependency, CellDependencyKind, CellPayload,
        Citation, CommitId, Confidence, Evidence, Scope, SemanticAnchor, SourceId, StateCell,
        StateCellId, TrustSignal, ValidTimeRange,
    };
    use continuitydb_kernel::{CellLookup, KernelError, StorageKernel};

    use super::MemoryKernel;

    fn test_commit_time() -> Result<chrono::DateTime<Utc>, Box<dyn std::error::Error>> {
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
        cell.system_time = continuitydb_core::SystemTimeRange::open_from(committed_at);
        cell.commit_id = commit_id;
        Ok(cell)
    }

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

        let cell = append_committed(&mut kernel, cell)?;
        let results = kernel.lookup_cells(CellLookup {
            semantic_anchor: Some("project:continuitydb:status".to_string()),
            ..CellLookup::default()
        })?;

        assert_eq!(results, vec![cell]);
        Ok(())
    }

    #[test]
    fn memory_kernel_appends_batch_with_shared_system_time(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let committed_at = test_commit_time()?;
        let first = sample_cell("project:continuitydb:batch-first", 0.9, 12)?;
        let second = sample_cell("project:continuitydb:batch-second", 0.8, 15)?;
        let first_id = first.id;
        let second_id = second.id;

        kernel.append_cells_at(vec![first, second], committed_at)?;

        let results = kernel.lookup_cells(CellLookup::default())?;
        assert_eq!(
            results.iter().map(|cell| cell.id).collect::<Vec<_>>(),
            vec![first_id, second_id]
        );
        assert!(results
            .iter()
            .all(|cell| cell.system_time.from() == committed_at));
        Ok(())
    }

    #[test]
    fn memory_kernel_rejects_duplicate_ids_inside_batch_without_partial_append(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let committed_at = test_commit_time()?;
        let cell = sample_cell("project:continuitydb:batch-duplicate", 0.9, 12)?;

        let result = kernel.append_cells_at(vec![cell.clone(), cell], committed_at);

        assert!(matches!(result, Err(KernelError::DuplicateCell)));
        assert!(kernel.lookup_cells(CellLookup::default())?.is_empty());
        Ok(())
    }

    #[test]
    fn memory_kernel_stamps_batch_with_explicit_commit_id() -> Result<(), Box<dyn std::error::Error>>
    {
        let mut kernel = MemoryKernel::default();
        let committed_at = test_commit_time()?;
        let commit_id = CommitId::new();
        let first = sample_cell("project:continuitydb:commit-first", 0.9, 12)?;
        let second = sample_cell("project:continuitydb:commit-second", 0.8, 15)?;
        let expected_ids = vec![first.id, second.id];

        kernel.append_cells_at_with_commit_id(vec![first, second], committed_at, commit_id)?;

        let results = kernel.lookup_cells(CellLookup {
            commit_id: Some(commit_id),
            ..CellLookup::default()
        })?;
        assert_eq!(
            results.iter().map(|cell| cell.id).collect::<Vec<_>>(),
            expected_ids
        );
        assert!(results.iter().all(|cell| cell.commit_id == commit_id));
        assert!(results
            .iter()
            .all(|cell| cell.system_time.from() == committed_at));
        Ok(())
    }

    #[test]
    fn memory_kernel_records_commit_manifest_for_batch() -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let committed_at = test_commit_time()?;
        let commit_id = CommitId::new();
        let first = sample_cell("project:continuitydb:manifest-first", 0.91, 12)?;
        let second = sample_cell("project:continuitydb:manifest-second", 0.83, 15)?;
        let expected_ids = vec![first.id, second.id];

        kernel.append_cells_at_with_commit_id(vec![first, second], committed_at, commit_id)?;

        let manifest = kernel
            .lookup_commit_manifest(commit_id)?
            .ok_or_else(|| std::io::Error::other("missing manifest"))?;
        assert_eq!(manifest.commit_id, commit_id);
        assert_eq!(manifest.committed_at, committed_at);
        assert_eq!(manifest.cell_ids, expected_ids);
        Ok(())
    }

    #[test]
    fn memory_kernel_rejects_duplicate_commit_id_without_partial_visibility(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let committed_at = test_commit_time()?;
        let commit_id = CommitId::new();
        let first = sample_cell("project:continuitydb:manifest-first", 0.91, 12)?;
        let second = sample_cell("project:continuitydb:manifest-second", 0.83, 15)?;

        kernel.append_cells_at_with_commit_id(vec![first.clone()], committed_at, commit_id)?;
        let result = kernel.append_cells_at_with_commit_id(vec![second], committed_at, commit_id);

        assert!(matches!(result, Err(KernelError::DuplicateCommit)));
        let stored = kernel.lookup_cells(CellLookup::default())?;
        assert_eq!(
            stored.iter().map(|cell| cell.id).collect::<Vec<_>>(),
            vec![first.id]
        );
        assert_eq!(
            kernel
                .lookup_commit_manifest(commit_id)?
                .ok_or_else(|| std::io::Error::other("missing manifest"))?
                .cell_ids,
            vec![first.id]
        );
        Ok(())
    }

    #[test]
    fn memory_kernel_lists_commit_manifests_in_append_order(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let first_time = test_commit_time()?;
        let second_time = Utc
            .with_ymd_and_hms(2026, 5, 20, 12, 30, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let first_commit = CommitId::new();
        let second_commit = CommitId::new();
        let first = sample_cell("project:continuitydb:list-first", 0.91, 12)?;
        let second = sample_cell("project:continuitydb:list-second", 0.83, 15)?;
        let expected_first_ids = vec![first.id];
        let expected_second_ids = vec![second.id];

        kernel.append_cells_at_with_commit_id(vec![first], first_time, first_commit)?;
        kernel.append_cells_at_with_commit_id(vec![second], second_time, second_commit)?;

        let manifests = kernel.list_commit_manifests()?;
        assert_eq!(manifests.len(), 2);
        assert_eq!(manifests[0].commit_id, first_commit);
        assert_eq!(manifests[0].committed_at, first_time);
        assert_eq!(manifests[0].cell_ids, expected_first_ids);
        assert_eq!(manifests[1].commit_id, second_commit);
        assert_eq!(manifests[1].committed_at, second_time);
        assert_eq!(manifests[1].cell_ids, expected_second_ids);
        Ok(())
    }

    #[test]
    fn memory_kernel_omits_empty_batches_from_commit_manifest_listing(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();

        kernel.append_cells_at_with_commit_id(Vec::new(), test_commit_time()?, CommitId::new())?;

        assert!(kernel.list_commit_manifests()?.is_empty());
        Ok(())
    }

    #[test]
    fn memory_kernel_filters_by_cell_id() -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let first = sample_cell("project:continuitydb:first", 0.9, 12)?;
        let second = sample_cell("project:continuitydb:second", 0.8, 15)?;
        append_committed(&mut kernel, first)?;
        let second = append_committed(&mut kernel, second)?;

        let results = kernel.lookup_cells(CellLookup {
            cell_id: Some(second.id),
            ..CellLookup::default()
        })?;

        assert_eq!(results, vec![second]);
        Ok(())
    }

    #[test]
    fn memory_kernel_filters_by_activation_state() -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let mut active = sample_cell("project:continuitydb:active", 0.9, 12)?;
        active.activation = ActivationState::Active;
        let mut frontier = sample_cell("project:continuitydb:frontier", 0.8, 15)?;
        frontier.activation = ActivationState::Frontier;
        append_committed(&mut kernel, active)?;
        let frontier = append_committed(&mut kernel, frontier)?;

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
        append_committed(&mut kernel, status)?;
        let frontier = append_committed(&mut kernel, frontier)?;

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
        append_committed(&mut kernel, observed)?;
        let reviewed = append_committed(&mut kernel, reviewed)?;

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
        append_committed(&mut kernel, weak)?;
        let strong = append_committed(&mut kernel, strong)?;

        let results = kernel.lookup_cells(CellLookup {
            minimum_confidence: Some(Confidence::new(0.8)?),
            ..CellLookup::default()
        })?;

        assert_eq!(results, vec![strong]);
        Ok(())
    }

    #[test]
    fn memory_kernel_filters_by_system_time() -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let committed_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let before_commit = Utc
            .with_ymd_and_hms(2026, 5, 20, 11, 59, 59)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let cell = sample_cell("project:continuitydb:system-time", 0.91, 12)?;
        kernel.append_cell_at(cell.clone(), committed_at)?;

        let current = kernel.lookup_cells(CellLookup {
            system_at: Some(committed_at),
            ..CellLookup::default()
        })?;
        let historical = kernel.lookup_cells(CellLookup {
            system_at: Some(before_commit),
            ..CellLookup::default()
        })?;

        assert_eq!(current.len(), 1);
        assert_eq!(current[0].id, cell.id);
        assert_eq!(current[0].system_time.from(), committed_at);
        assert!(historical.is_empty());
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
        let dependent = append_committed(&mut kernel, dependent)?;
        append_committed(&mut kernel, unrelated)?;
        append_committed(&mut kernel, support)?;

        let results = kernel.lookup_cells(CellLookup {
            dependency_target: Some(target),
            dependency_kind: Some(CellDependencyKind::DependsOn),
            ..CellLookup::default()
        })?;

        assert_eq!(results, vec![dependent]);
        Ok(())
    }
}
