//! In-memory StorageKernel implementation for correctness tests.

use chrono::{DateTime, Utc};
use continuitydb_core::{
    CommitId, CommitManifest, ContextPacketSelectionReason, RevisionLinkRecord, StateCell,
    SystemTimeRange, UtilityFeedback,
};
use continuitydb_kernel::{
    CellLookup, CommitManifestLookup, KernelError, RevisionLinkLookup, StorageKernel,
};
use std::collections::{HashMap, HashSet};

/// Append-only in-memory storage kernel.
#[derive(Default)]
pub struct MemoryKernel {
    cells: Vec<StateCell>,
    manifests: HashMap<CommitId, CommitManifest>,
    manifest_order: Vec<CommitId>,
    revision_links: Vec<RevisionLinkRecord>,
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
                    .lifecycle_stage
                    .map_or(true, |stage| cell.lifecycle_stage == stage)
            })
            .filter(|cell| {
                lookup
                    .retention_policy
                    .map_or(true, |policy| cell.lifecycle_policy.retention == policy)
            })
            .filter(|cell| {
                lookup
                    .use_policy
                    .map_or(true, |policy| cell.lifecycle_policy.use_policy == policy)
            })
            .filter(|cell| {
                lookup
                    .promotion_policy
                    .map_or(true, |policy| cell.lifecycle_policy.promotion == policy)
            })
            .filter(|cell| {
                lookup.projection_kind.map_or(true, |kind| {
                    cell.projections
                        .iter()
                        .any(|projection| projection.kind == kind)
                })
            })
            .filter(|cell| {
                lookup.minimum_uncertainty.map_or(true, |minimum| {
                    cell.uncertainty.score.value() >= minimum.value()
                })
            })
            .filter(|cell| {
                lookup
                    .minimum_surprise_bits
                    .map_or(true, |minimum| cell.uncertainty.surprise_bits >= minimum)
            })
            .filter(|cell| {
                lookup.minimum_probability_delta.map_or(true, |minimum| {
                    cell.uncertainty
                        .expectation
                        .as_ref()
                        .is_some_and(|expectation| expectation.probability_delta >= minimum)
                })
            })
            .filter(|cell| {
                lookup
                    .minimum_salience
                    .map_or(true, |minimum| cell.attention.salience_score() >= minimum)
            })
            .filter(|cell| {
                lookup.minimum_context_affordance.map_or(true, |minimum| {
                    cell.context_affordance.context_affordance_score() >= minimum
                })
            })
            .filter(|cell| {
                lookup.minimum_epistemic_pressure.map_or(true, |minimum| {
                    cell.epistemic_pressure().checkout_pressure >= minimum
                })
            })
            .filter(|cell| {
                lookup.context_gap_kind.map_or(true, |kind| {
                    cell.context_gaps.iter().any(|gap| gap.kind == kind)
                })
            })
            .filter(|cell| {
                lookup.minimum_context_gap_priority.map_or(true, |minimum| {
                    cell.context_gaps
                        .iter()
                        .any(|gap| gap.priority.value() >= minimum.value())
                })
            })
            .filter(|cell| {
                lookup.invalidation_condition_kind.map_or(true, |kind| {
                    cell.invalidation_conditions
                        .iter()
                        .any(|condition| condition.kind == kind)
                })
            })
            .filter(|cell| {
                lookup
                    .minimum_invalidation_priority
                    .map_or(true, |minimum| {
                        cell.invalidation_conditions
                            .iter()
                            .any(|condition| condition.priority.value() >= minimum.value())
                    })
            })
            .filter(|cell| {
                lookup
                    .epistemic_action
                    .map_or(true, |action| cell.epistemic_action() == action)
            })
            .filter(|cell| {
                lookup.epistemic_action_reason.map_or(true, |reason| {
                    cell.epistemic_action_reasons().contains(&reason)
                })
            })
            .filter(|cell| {
                lookup
                    .selection_reason
                    .map_or(true, |reason| cell_matches_selection_reason(cell, reason))
            })
            .filter(|cell| {
                lookup.trajectory_memory_strategy.map_or(true, |strategy| {
                    cell.trajectory_memory
                        .as_ref()
                        .is_some_and(|memory| memory.checkout_strategy == strategy)
                })
            })
            .filter(|cell| {
                lookup
                    .minimum_trajectory_memory_confidence
                    .map_or(true, |minimum| {
                        cell.trajectory_memory
                            .as_ref()
                            .is_some_and(|memory| memory.confidence.value() >= minimum.value())
                    })
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
                if lookup.dependency_target.is_none() && lookup.dependency_kind.is_none() {
                    return true;
                }
                cell.dependencies.iter().any(|dependency| {
                    lookup
                        .dependency_target
                        .map_or(true, |target| dependency.target == target)
                        && lookup
                            .dependency_kind
                            .map_or(true, |kind| dependency.kind == kind)
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
        self.list_commit_manifests_matching(CommitManifestLookup::default())
    }

    fn list_commit_manifests_matching(
        &self,
        lookup: CommitManifestLookup,
    ) -> Result<Vec<CommitManifest>, KernelError> {
        let start = if let Some(after) = lookup.after {
            self.manifest_order
                .iter()
                .position(|commit_id| *commit_id == after)
                .map(|position| position + 1)
                .ok_or(KernelError::CommitNotFound)?
        } else {
            0
        };
        let limit = lookup.limit.unwrap_or(usize::MAX);
        Ok(self
            .manifest_order
            .iter()
            .skip(start)
            .take(limit)
            .filter_map(|commit_id| self.manifests.get(commit_id).cloned())
            .collect())
    }

    fn append_revision_link(
        &mut self,
        revision_link: RevisionLinkRecord,
    ) -> Result<(), KernelError> {
        if self.revision_links.contains(&revision_link) {
            return Err(KernelError::DuplicateRevisionLink);
        }

        self.revision_links.push(revision_link);
        Ok(())
    }

    fn list_revision_links(
        &self,
        lookup: RevisionLinkLookup,
    ) -> Result<Vec<RevisionLinkRecord>, KernelError> {
        Ok(self
            .revision_links
            .iter()
            .filter(|revision_link| {
                lookup
                    .source
                    .map_or(true, |source| revision_link.source == source)
            })
            .filter(|revision_link| {
                lookup
                    .target
                    .map_or(true, |target| revision_link.target == target)
            })
            .filter(|revision_link| lookup.kind.map_or(true, |kind| revision_link.kind == kind))
            .cloned()
            .collect())
    }
}

fn cell_matches_selection_reason(cell: &StateCell, reason: ContextPacketSelectionReason) -> bool {
    match reason {
        ContextPacketSelectionReason::EvidenceConfidence => cell
            .evidence
            .iter()
            .any(|evidence| evidence.confidence.value() > 0.0),
        ContextPacketSelectionReason::UtilityFeedback => {
            cell.utility_feedback.utility_score() != UtilityFeedback::default().utility_score()
        }
        ContextPacketSelectionReason::LifecycleStage => cell.lifecycle_stage != Default::default(),
        ContextPacketSelectionReason::ProjectionProfile => !cell.projections.is_empty(),
        ContextPacketSelectionReason::NativeUncertainty => cell.uncertainty.is_recorded(),
        ContextPacketSelectionReason::EpistemicCalibration => cell.calibration.is_recorded(),
        ContextPacketSelectionReason::ContextAffordance => cell.context_affordance.is_recorded(),
        ContextPacketSelectionReason::ContextGap => !cell.context_gaps.is_empty(),
        ContextPacketSelectionReason::InvalidationCondition => {
            !cell.invalidation_conditions.is_empty()
        }
        ContextPacketSelectionReason::TrajectoryMemory => cell.trajectory_memory.is_some(),
        ContextPacketSelectionReason::LifecyclePolicy => {
            cell.lifecycle_policy != Default::default()
        }
        ContextPacketSelectionReason::AttentionSignal => cell.attention.is_recorded(),
        ContextPacketSelectionReason::Answerability => !cell.answerability.questions().is_empty(),
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use continuitydb_core::{
        ActivationState, Answerability, AttentionSignal, CellCost, CellDependency,
        CellDependencyKind, CellPayload, Citation, CommitId, Confidence, ContextAffordance,
        ContextGap, ContextGapKind, ContextLifecyclePolicy, ContextPacketSelectionReason,
        ContextPacketStrategy, EpistemicAction, EpistemicActionReason, EpistemicExpectation,
        EpistemicUncertainty, Evidence, InvalidationCondition, InvalidationConditionKind,
        LifecycleStage, MemoryProjection, MemoryProjectionKind, PromotionPolicy, RetentionPolicy,
        RevisionLinkKind, RevisionLinkRecord, Scope, SemanticAnchor, SourceId, StateCell,
        StateCellId, TrajectoryMemory, TrustSignal, UsePolicy, ValidTimeRange,
    };
    use continuitydb_kernel::{
        CellLookup, CommitManifestLookup, KernelDurability, KernelError, RevisionLinkLookup,
        StorageKernel,
    };

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
    fn memory_kernel_reports_ephemeral_capabilities() {
        let kernel = MemoryKernel::default();
        let capabilities = kernel.capabilities();

        assert_eq!(KernelDurability::Ephemeral, capabilities.durability);
        assert!(capabilities.append_only);
        assert!(!capabilities.derived_indexes);
        assert!(!capabilities.persistent_indexes);
        assert!(!capabilities.explicit_commit_records);
        assert!(!capabilities.durable_flush);
        assert!(!capabilities.compaction);
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
    fn revision_link_storage_memory_kernel_appends_and_filters_links(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let first = RevisionLinkRecord::new(
            StateCellId::from_u128(1),
            RevisionLinkKind::Supersedes,
            StateCellId::from_u128(2),
            test_commit_time()?,
        );
        let second = RevisionLinkRecord::new(
            StateCellId::from_u128(1),
            RevisionLinkKind::ConflictsWith,
            StateCellId::from_u128(3),
            test_commit_time()?,
        );

        kernel.append_revision_link(first.clone())?;
        kernel.append_revision_link(second.clone())?;

        assert_eq!(
            kernel.list_revision_links(RevisionLinkLookup::default())?,
            vec![first.clone(), second]
        );
        assert_eq!(
            kernel.list_revision_links(RevisionLinkLookup {
                source: Some(first.source),
                target: Some(first.target),
                kind: Some(RevisionLinkKind::Supersedes),
            })?,
            vec![first]
        );
        Ok(())
    }

    #[test]
    fn revision_link_storage_memory_kernel_rejects_duplicate_links(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let record = RevisionLinkRecord::new(
            StateCellId::from_u128(1),
            RevisionLinkKind::Supersedes,
            StateCellId::from_u128(2),
            test_commit_time()?,
        );

        kernel.append_revision_link(record.clone())?;
        let result = kernel.append_revision_link(record.clone());

        assert!(matches!(result, Err(KernelError::DuplicateRevisionLink)));
        assert_eq!(
            kernel.list_revision_links(RevisionLinkLookup::default())?,
            vec![record]
        );
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
    fn memory_kernel_lists_commit_manifests_after_cursor() -> Result<(), Box<dyn std::error::Error>>
    {
        let mut kernel = MemoryKernel::default();
        let first_time = test_commit_time()?;
        let second_time = Utc
            .with_ymd_and_hms(2026, 5, 20, 12, 30, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let third_time = Utc
            .with_ymd_and_hms(2026, 5, 20, 13, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let first_commit = CommitId::new();
        let second_commit = CommitId::new();
        let third_commit = CommitId::new();

        kernel.append_cells_at_with_commit_id(
            vec![sample_cell("project:continuitydb:cursor-first", 0.91, 12)?],
            first_time,
            first_commit,
        )?;
        kernel.append_cells_at_with_commit_id(
            vec![sample_cell("project:continuitydb:cursor-second", 0.83, 15)?],
            second_time,
            second_commit,
        )?;
        kernel.append_cells_at_with_commit_id(
            vec![sample_cell("project:continuitydb:cursor-third", 0.77, 18)?],
            third_time,
            third_commit,
        )?;

        let manifests = kernel.list_commit_manifests_matching(CommitManifestLookup {
            after: Some(first_commit),
            limit: None,
        })?;

        assert_eq!(
            manifests
                .iter()
                .map(|manifest| manifest.commit_id)
                .collect::<Vec<_>>(),
            vec![second_commit, third_commit]
        );
        Ok(())
    }

    #[test]
    fn memory_kernel_limits_commit_manifest_listing() -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let first_commit = CommitId::new();
        let second_commit = CommitId::new();
        kernel.append_cells_at_with_commit_id(
            vec![sample_cell("project:continuitydb:limit-first", 0.91, 12)?],
            test_commit_time()?,
            first_commit,
        )?;
        kernel.append_cells_at_with_commit_id(
            vec![sample_cell("project:continuitydb:limit-second", 0.83, 15)?],
            Utc.with_ymd_and_hms(2026, 5, 20, 12, 30, 0)
                .single()
                .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?,
            second_commit,
        )?;

        let manifests = kernel.list_commit_manifests_matching(CommitManifestLookup {
            after: None,
            limit: Some(1),
        })?;

        assert_eq!(manifests.len(), 1);
        assert_eq!(manifests[0].commit_id, first_commit);
        Ok(())
    }

    #[test]
    fn memory_kernel_reports_unknown_commit_manifest_cursor(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let kernel = MemoryKernel::default();

        let result = kernel.list_commit_manifests_matching(CommitManifestLookup {
            after: Some(CommitId::new()),
            limit: None,
        });

        assert!(matches!(result, Err(KernelError::CommitNotFound)));
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
    fn memory_kernel_filters_by_lifecycle_stage() -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let observed = append_committed(
            &mut kernel,
            sample_cell("project:continuitydb:lifecycle-observed", 0.8, 10)?,
        )?;
        let consolidated = append_committed(
            &mut kernel,
            sample_cell("project:continuitydb:lifecycle-consolidated", 0.9, 10)?
                .with_lifecycle_stage(LifecycleStage::Consolidated),
        )?;

        let results = kernel.lookup_cells(CellLookup {
            lifecycle_stage: Some(LifecycleStage::Consolidated),
            ..CellLookup::default()
        })?;

        assert_eq!(results, vec![consolidated]);
        assert_ne!(results, vec![observed]);
        Ok(())
    }

    #[test]
    fn memory_kernel_filters_by_lifecycle_use_policy() -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let mut verify = sample_cell("project:continuitydb:lifecycle-policy-verify", 0.9, 10)?;
        verify.set_lifecycle_policy(ContextLifecyclePolicy {
            retention: RetentionPolicy::DecayUnlessReinforced,
            use_policy: UsePolicy::VerifyBeforeUse,
            promotion: PromotionPolicy::Manual,
        });
        let verify = append_committed(&mut kernel, verify)?;
        append_committed(
            &mut kernel,
            sample_cell("project:continuitydb:lifecycle-policy-use", 0.9, 10)?,
        )?;

        let results = kernel.lookup_cells(CellLookup {
            use_policy: Some(UsePolicy::VerifyBeforeUse),
            ..CellLookup::default()
        })?;

        assert_eq!(results, vec![verify]);
        Ok(())
    }

    #[test]
    fn memory_kernel_filters_by_projection_kind() -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let mut procedural = sample_cell("project:continuitydb:projection-procedural", 0.9, 12)?;
        procedural.add_projection(MemoryProjection::new(
            MemoryProjectionKind::Procedural,
            "Run the retained validation command before trusting artifacts.",
            Confidence::new(0.91)?,
            CellCost::new(8, 0)?,
        )?);
        let procedural = append_committed(&mut kernel, procedural)?;
        let mut semantic = sample_cell("project:continuitydb:projection-semantic", 0.9, 12)?;
        semantic.add_projection(MemoryProjection::new(
            MemoryProjectionKind::Semantic,
            "Artifact validation proves retained bundle integrity.",
            Confidence::new(0.88)?,
            CellCost::new(8, 0)?,
        )?);
        append_committed(&mut kernel, semantic)?;

        let results = kernel.lookup_cells(CellLookup {
            projection_kind: Some(MemoryProjectionKind::Procedural),
            ..CellLookup::default()
        })?;

        assert_eq!(results, vec![procedural]);
        Ok(())
    }

    #[test]
    fn memory_kernel_filters_by_uncertainty_thresholds() -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let mut surprising = sample_cell("project:continuitydb:uncertainty-surprising", 0.9, 12)?;
        surprising.set_uncertainty(EpistemicUncertainty::new(
            Confidence::new(0.76)?,
            4.2,
            "baseline belief failed",
        )?);
        let surprising = append_committed(&mut kernel, surprising)?;
        let mut routine = sample_cell("project:continuitydb:uncertainty-routine", 0.9, 12)?;
        routine.set_uncertainty(EpistemicUncertainty::new(
            Confidence::new(0.4)?,
            0.8,
            "minor ambiguity",
        )?);
        append_committed(&mut kernel, routine)?;

        let results = kernel.lookup_cells(CellLookup {
            minimum_uncertainty: Some(Confidence::new(0.7)?),
            minimum_surprise_bits: Some(3.0),
            ..CellLookup::default()
        })?;

        assert_eq!(results, vec![surprising]);
        Ok(())
    }

    #[test]
    fn memory_kernel_filters_by_minimum_salience() -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let mut salient = sample_cell("project:continuitydb:salient-context", 0.9, 12)?;
        salient.set_attention(AttentionSignal::new(0.9, 0.8, 0.9, 0.8)?);
        let salient = append_committed(&mut kernel, salient)?;
        let mut routine = sample_cell("project:continuitydb:routine-context", 0.9, 12)?;
        routine.set_attention(AttentionSignal::new(0.1, 0.1, 0.2, 0.0)?);
        append_committed(&mut kernel, routine)?;

        let results = kernel.lookup_cells(CellLookup {
            minimum_salience: Some(0.7),
            ..CellLookup::default()
        })?;

        assert_eq!(results, vec![salient]);
        Ok(())
    }

    #[test]
    fn memory_kernel_filters_by_minimum_context_affordance(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let mut high_value = sample_cell("project:continuitydb:high-affordance-context", 0.9, 12)?;
        high_value.set_context_affordance(ContextAffordance::new(0.95, 0.9, 0.8, 0.4, 0.95, 0.1)?);
        let high_value = append_committed(&mut kernel, high_value)?;
        let mut routine = sample_cell("project:continuitydb:routine-affordance-context", 0.9, 12)?;
        routine.set_context_affordance(ContextAffordance::new(0.2, 0.1, 0.1, 0.1, 0.2, 0.2)?);
        append_committed(&mut kernel, routine)?;

        let results = kernel.lookup_cells(CellLookup {
            minimum_context_affordance: Some(0.7),
            ..CellLookup::default()
        })?;

        assert_eq!(results, vec![high_value]);
        Ok(())
    }

    #[test]
    fn memory_kernel_filters_by_minimum_epistemic_pressure(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let mut pressured = sample_cell("project:continuitydb:pressure-context", 0.8, 12)?;
        pressured.set_uncertainty(EpistemicUncertainty::new(
            Confidence::new(0.7)?,
            4.0,
            "high uncertainty with violated baseline",
        )?);
        pressured.set_attention(AttentionSignal::new(0.8, 0.6, 0.75, 0.4)?);
        let pressured = append_committed(&mut kernel, pressured)?;
        let mut routine = sample_cell("project:continuitydb:routine-pressure-context", 0.9, 12)?;
        routine.set_uncertainty(EpistemicUncertainty::new(
            Confidence::new(0.2)?,
            0.2,
            "minor uncertainty",
        )?);
        append_committed(&mut kernel, routine)?;

        let results = kernel.lookup_cells(CellLookup {
            minimum_epistemic_pressure: Some(0.55),
            ..CellLookup::default()
        })?;

        assert_eq!(results, vec![pressured]);
        Ok(())
    }

    #[test]
    fn memory_kernel_filters_by_minimum_probability_delta() -> Result<(), Box<dyn std::error::Error>>
    {
        let mut kernel = MemoryKernel::default();
        let mut shifted = sample_cell("project:continuitydb:probability-shift", 0.9, 12)?;
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
        let mut routine = sample_cell("project:continuitydb:probability-routine", 0.9, 12)?;
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

        let results = kernel.lookup_cells(CellLookup {
            minimum_probability_delta: Some(0.7),
            ..CellLookup::default()
        })?;

        assert_eq!(results, vec![shifted]);
        Ok(())
    }

    #[test]
    fn memory_kernel_filters_by_epistemic_action() -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let mut scavenge = sample_cell("project:continuitydb:action-scavenge", 0.9, 12)?;
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
        append_committed(
            &mut kernel,
            sample_cell("project:continuitydb:action-use", 0.9, 12)?,
        )?;

        let results = kernel.lookup_cells(CellLookup {
            epistemic_action: Some(EpistemicAction::Scavenge),
            ..CellLookup::default()
        })?;

        assert_eq!(results, vec![scavenge]);
        Ok(())
    }

    #[test]
    fn memory_kernel_filters_by_epistemic_action_reason() -> Result<(), Box<dyn std::error::Error>>
    {
        let mut kernel = MemoryKernel::default();
        let mut surprising = sample_cell("project:continuitydb:reason-surprise", 0.9, 12)?;
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
        let mut uncertain = sample_cell("project:continuitydb:reason-uncertainty", 0.9, 12)?;
        uncertain.set_uncertainty(EpistemicUncertainty::new(
            Confidence::new(0.82)?,
            0.5,
            "uncertain but not surprising",
        )?);
        append_committed(&mut kernel, uncertain)?;

        let results = kernel.lookup_cells(CellLookup {
            epistemic_action_reason: Some(EpistemicActionReason::HighSurprise),
            ..CellLookup::default()
        })?;

        assert_eq!(results, vec![surprising]);
        Ok(())
    }

    #[test]
    fn memory_kernel_filters_by_selection_reason() -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let routine = append_committed(
            &mut kernel,
            sample_cell("project:continuitydb:selection-reason-routine", 0.9, 12)?,
        )?;
        let mut salient = sample_cell("project:continuitydb:selection-reason-attention", 0.9, 12)?;
        salient.set_attention(AttentionSignal::new(0.8, 0.7, 0.9, 0.6)?);
        let salient = append_committed(&mut kernel, salient)?;

        let results = kernel.lookup_cells(CellLookup {
            selection_reason: Some(ContextPacketSelectionReason::AttentionSignal),
            ..CellLookup::default()
        })?;

        assert_eq!(results, vec![salient]);
        assert!(!results.contains(&routine));
        Ok(())
    }

    #[test]
    fn memory_kernel_filters_by_trajectory_memory_confidence_and_strategy(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let mut reusable = sample_cell("project:continuitydb:trajectory-memory-reusable", 0.9, 12)?;
        reusable.set_trajectory_memory(TrajectoryMemory::new(
            "retry release upload without checking target release",
            "found package artifact and upload command",
            "GitHub release upload returned 404",
            "target/alpha-workflow/release-upload-failure.json",
            0.86,
            "verify the release target before reusing upload state",
            vec!["release upload workflow".to_string()],
            vec!["successful upload report exists".to_string()],
            ContextPacketStrategy::FalsificationBrief,
        )?);
        let reusable = append_committed(&mut kernel, reusable)?;
        let mut weak = sample_cell("project:continuitydb:trajectory-memory-weak", 0.9, 12)?;
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
        let mut wrong_shape = sample_cell(
            "project:continuitydb:trajectory-memory-wrong-shape",
            0.9,
            12,
        )?;
        wrong_shape.set_trajectory_memory(TrajectoryMemory::new(
            "look for missing release evidence",
            "found partial logs",
            "trace lacks upload outcome",
            "target/scavenge-trace.json",
            0.91,
            "scavenge for retained upload evidence first",
            vec!["release upload workflow".to_string()],
            vec!["complete upload report exists".to_string()],
            ContextPacketStrategy::ScavengingBrief,
        )?);
        append_committed(&mut kernel, wrong_shape)?;

        let results = kernel.lookup_cells(CellLookup {
            trajectory_memory_strategy: Some(ContextPacketStrategy::FalsificationBrief),
            minimum_trajectory_memory_confidence: Some(Confidence::new(0.8)?),
            ..CellLookup::default()
        })?;

        assert_eq!(results, vec![reusable]);
        Ok(())
    }

    #[test]
    fn memory_kernel_filters_by_context_gap_kind_and_priority(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let routine = append_committed(
            &mut kernel,
            sample_cell("project:continuitydb:context-gap-routine", 0.9, 12)?,
        )?;
        let mut low_priority =
            sample_cell("project:continuitydb:context-gap-low-priority", 0.9, 12)?;
        low_priority.add_context_gap(ContextGap::new(
            ContextGapKind::MissingEvidence,
            "which artifact proves the claim?",
            "low-priority gap should not cross the threshold",
            0.4,
        )?);
        let low_priority = append_committed(&mut kernel, low_priority)?;
        let mut high_priority =
            sample_cell("project:continuitydb:context-gap-high-priority", 0.9, 12)?;
        high_priority.add_context_gap(ContextGap::new(
            ContextGapKind::MissingEvidence,
            "which retained artifact proves the live run?",
            "high-priority missing evidence should remain retrievable",
            0.9,
        )?);
        let high_priority = append_committed(&mut kernel, high_priority)?;

        let results = kernel.lookup_cells(CellLookup {
            context_gap_kind: Some(ContextGapKind::MissingEvidence),
            minimum_context_gap_priority: Some(Confidence::new(0.7)?),
            ..CellLookup::default()
        })?;

        assert_eq!(results, vec![high_priority]);
        assert!(!results.contains(&routine));
        assert!(!results.contains(&low_priority));
        Ok(())
    }

    #[test]
    fn memory_kernel_filters_by_invalidation_condition_kind_and_priority(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let routine = append_committed(
            &mut kernel,
            sample_cell("project:continuitydb:invalidation-routine", 0.9, 12)?,
        )?;
        let mut low_priority =
            sample_cell("project:continuitydb:invalidation-low-priority", 0.9, 12)?;
        low_priority.add_invalidation_condition(InvalidationCondition::new(
            InvalidationConditionKind::DependencyInvalidated,
            "a low-impact dependency is superseded",
            "low-priority falsifiers should not cross the threshold",
            0.4,
        )?);
        let low_priority = append_committed(&mut kernel, low_priority)?;
        let mut high_priority =
            sample_cell("project:continuitydb:invalidation-high-priority", 0.9, 12)?;
        high_priority.add_invalidation_condition(InvalidationCondition::new(
            InvalidationConditionKind::DependencyInvalidated,
            "a retained dependency artifact is superseded",
            "high-priority falsifiers should remain directly retrievable",
            0.9,
        )?);
        let high_priority = append_committed(&mut kernel, high_priority)?;

        let results = kernel.lookup_cells(CellLookup {
            invalidation_condition_kind: Some(InvalidationConditionKind::DependencyInvalidated),
            minimum_invalidation_priority: Some(Confidence::new(0.7)?),
            ..CellLookup::default()
        })?;

        assert_eq!(results, vec![high_priority]);
        assert!(!results.contains(&routine));
        assert!(!results.contains(&low_priority));
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

    #[test]
    fn memory_kernel_filters_by_dependency_kind_without_target(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let target = StateCellId::new();
        let other_target = StateCellId::new();
        let mut first = sample_cell("project:continuitydb:kind-first", 0.9, 12)?;
        first.dependencies.push(CellDependency::new(
            target,
            CellDependencyKind::DependsOn,
            "depends on target",
        ));
        let mut support = sample_cell("project:continuitydb:kind-support", 0.9, 12)?;
        support.dependencies.push(CellDependency::new(
            target,
            CellDependencyKind::Supports,
            "supports target",
        ));
        let mut second = sample_cell("project:continuitydb:kind-second", 0.9, 12)?;
        second.dependencies.push(CellDependency::new(
            other_target,
            CellDependencyKind::DependsOn,
            "depends on another target",
        ));
        let first = append_committed(&mut kernel, first)?;
        append_committed(&mut kernel, support)?;
        let second = append_committed(&mut kernel, second)?;

        let results = kernel.lookup_cells(CellLookup {
            dependency_kind: Some(CellDependencyKind::DependsOn),
            ..CellLookup::default()
        })?;

        assert_eq!(results, vec![first, second]);
        Ok(())
    }
}
