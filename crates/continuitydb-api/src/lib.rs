//! Native embeddable operation API for ContinuityDB.

use chrono::{DateTime, Utc};
use continuitydb_checkout::{
    audit, checkout, AuditTrace, CheckoutError, CheckoutRequest, CheckoutSlice,
};
use continuitydb_core::{CommitId, CommitManifest, StateCell, StateCellId, UtilityFeedback};
use continuitydb_kernel::{CellLookup, CommitManifestLookup, KernelError, StorageKernel};
use continuitydb_revision::{
    detect_cell_conflict, recommend_conflict_resolution, recommend_conflict_resolutions,
    revise_utility_feedback, scan_cell_conflicts, CellConflict, CellConflictScan,
    ConflictResolutionRecommendation, ConflictResolutionScan,
};
use thiserror::Error;

/// Errors produced by the native ContinuityDB operation API.
#[derive(Debug, Error, PartialEq)]
pub enum ContinuityError {
    /// Storage kernel failure.
    #[error(transparent)]
    Kernel(#[from] KernelError),
    /// Checkout operation failure.
    #[error(transparent)]
    Checkout(#[from] CheckoutError),
    /// Requested StateCell was not found in the backing kernel.
    #[error("state cell not found")]
    CellNotFound {
        /// Missing cell identifier.
        cell_id: StateCellId,
    },
}

/// Native embeddable ContinuityDB operation boundary.
#[derive(Clone, Debug)]
pub struct ContinuityDb<K> {
    kernel: K,
}

impl<K> ContinuityDb<K> {
    /// Creates a database wrapper over an existing storage kernel.
    pub fn new(kernel: K) -> Self {
        Self { kernel }
    }

    /// Returns the backing storage kernel.
    pub fn kernel(&self) -> &K {
        &self.kernel
    }

    /// Returns mutable access to the backing storage kernel.
    pub fn kernel_mut(&mut self) -> &mut K {
        &mut self.kernel
    }

    /// Consumes the API wrapper and returns the backing storage kernel.
    pub fn into_kernel(self) -> K {
        self.kernel
    }
}

impl<K: StorageKernel> ContinuityDb<K> {
    /// Appends an immutable StateCell version and returns its identifier.
    pub fn ingest_cell(&mut self, cell: StateCell) -> Result<StateCellId, ContinuityError> {
        let cell_id = cell.id;
        self.kernel.append_cell(cell)?;
        Ok(cell_id)
    }

    /// Appends an immutable StateCell version at a deterministic system time.
    pub fn ingest_cell_at(
        &mut self,
        cell: StateCell,
        committed_at: DateTime<Utc>,
    ) -> Result<StateCellId, ContinuityError> {
        let cell_id = cell.id;
        self.kernel.append_cell_at(cell, committed_at)?;
        Ok(cell_id)
    }

    /// Appends an immutable StateCell version at a deterministic system time and commit ID.
    pub fn ingest_cell_at_with_commit_id(
        &mut self,
        cell: StateCell,
        committed_at: DateTime<Utc>,
        commit_id: CommitId,
    ) -> Result<StateCellId, ContinuityError> {
        let cell_id = cell.id;
        self.kernel
            .append_cell_at_with_commit_id(cell, committed_at, commit_id)?;
        Ok(cell_id)
    }

    /// Appends immutable StateCell versions as one batch and returns their identifiers.
    pub fn ingest_cells<I>(&mut self, cells: I) -> Result<Vec<StateCellId>, ContinuityError>
    where
        I: IntoIterator<Item = StateCell>,
    {
        self.ingest_cells_at(cells, Utc::now())
    }

    /// Appends immutable StateCell versions as one batch at a deterministic system time.
    pub fn ingest_cells_at<I>(
        &mut self,
        cells: I,
        committed_at: DateTime<Utc>,
    ) -> Result<Vec<StateCellId>, ContinuityError>
    where
        I: IntoIterator<Item = StateCell>,
    {
        let cells = cells.into_iter().collect::<Vec<_>>();
        let cell_ids = cells.iter().map(|cell| cell.id).collect::<Vec<_>>();
        self.kernel.append_cells_at(cells, committed_at)?;
        Ok(cell_ids)
    }

    /// Appends immutable StateCell versions as one batch at a deterministic system time and commit ID.
    pub fn ingest_cells_at_with_commit_id<I>(
        &mut self,
        cells: I,
        committed_at: DateTime<Utc>,
        commit_id: CommitId,
    ) -> Result<Vec<StateCellId>, ContinuityError>
    where
        I: IntoIterator<Item = StateCell>,
    {
        let cells = cells.into_iter().collect::<Vec<_>>();
        let cell_ids = cells.iter().map(|cell| cell.id).collect::<Vec<_>>();
        self.kernel
            .append_cells_at_with_commit_id(cells, committed_at, commit_id)?;
        Ok(cell_ids)
    }

    /// Materializes a deterministic continuity slice.
    pub fn checkout(&self, request: CheckoutRequest) -> Result<CheckoutSlice, ContinuityError> {
        checkout(&self.kernel, request).map_err(Into::into)
    }

    /// Returns the manifest for a database commit boundary when it exists.
    pub fn commit_manifest(
        &self,
        commit_id: CommitId,
    ) -> Result<Option<CommitManifest>, ContinuityError> {
        self.kernel
            .lookup_commit_manifest(commit_id)
            .map_err(Into::into)
    }

    /// Returns commit manifests in database visibility order.
    pub fn commit_manifests(&self) -> Result<Vec<CommitManifest>, ContinuityError> {
        self.kernel.list_commit_manifests().map_err(Into::into)
    }

    /// Returns commit manifests matching deterministic listing constraints.
    pub fn commit_manifests_matching(
        &self,
        lookup: CommitManifestLookup,
    ) -> Result<Vec<CommitManifest>, ContinuityError> {
        self.kernel
            .list_commit_manifests_matching(lookup)
            .map_err(Into::into)
    }

    /// Records utility feedback as an append-only successor StateCell.
    pub fn record_utility_feedback(
        &mut self,
        cell_id: StateCellId,
        feedback: UtilityFeedback,
    ) -> Result<StateCellId, ContinuityError> {
        self.record_utility_feedback_at(cell_id, feedback, Utc::now())
    }

    /// Records utility feedback as an append-only successor at a deterministic system time.
    pub fn record_utility_feedback_at(
        &mut self,
        cell_id: StateCellId,
        feedback: UtilityFeedback,
        committed_at: DateTime<Utc>,
    ) -> Result<StateCellId, ContinuityError> {
        let previous = self.lookup_one_cell(cell_id)?;
        let revision = revise_utility_feedback(&previous, feedback);
        let successor_id = revision.cell.id;
        self.kernel.append_cell_at(revision.cell, committed_at)?;
        Ok(successor_id)
    }

    /// Detects a deterministic conflict between two stored StateCells.
    pub fn detect_conflict(
        &self,
        left_id: StateCellId,
        right_id: StateCellId,
    ) -> Result<Option<CellConflict>, ContinuityError> {
        let left = self.lookup_one_cell(left_id)?;
        let right = self.lookup_one_cell(right_id)?;
        Ok(detect_cell_conflict(&left, &right))
    }

    /// Recommends a deterministic non-mutating conflict resolution for two stored StateCells.
    pub fn recommend_conflict_resolution(
        &self,
        left_id: StateCellId,
        right_id: StateCellId,
    ) -> Result<Option<ConflictResolutionRecommendation>, ContinuityError> {
        let left = self.lookup_one_cell(left_id)?;
        let right = self.lookup_one_cell(right_id)?;
        Ok(recommend_conflict_resolution(&left, &right))
    }

    /// Detects deterministic conflicts across a stored StateCell set.
    pub fn detect_conflicts<I>(&self, cell_ids: I) -> Result<CellConflictScan, ContinuityError>
    where
        I: IntoIterator<Item = StateCellId>,
    {
        let cells = self.lookup_cells_in_order(cell_ids)?;
        Ok(scan_cell_conflicts(&cells))
    }

    /// Recommends deterministic non-mutating resolutions across a stored StateCell set.
    pub fn recommend_conflict_resolutions<I>(
        &self,
        cell_ids: I,
    ) -> Result<ConflictResolutionScan, ContinuityError>
    where
        I: IntoIterator<Item = StateCellId>,
    {
        let cells = self.lookup_cells_in_order(cell_ids)?;
        Ok(recommend_conflict_resolutions(&cells))
    }

    /// Produces an audit trace for a stored StateCell.
    pub fn audit_cell(&self, cell_id: StateCellId) -> Result<AuditTrace, ContinuityError> {
        self.lookup_one_cell(cell_id).map(|cell| audit(&cell))
    }

    fn lookup_cells_in_order<I>(&self, cell_ids: I) -> Result<Vec<StateCell>, ContinuityError>
    where
        I: IntoIterator<Item = StateCellId>,
    {
        cell_ids
            .into_iter()
            .map(|cell_id| self.lookup_one_cell(cell_id))
            .collect()
    }

    fn lookup_one_cell(&self, cell_id: StateCellId) -> Result<StateCell, ContinuityError> {
        let cells = self.kernel.lookup_cells(CellLookup {
            cell_id: Some(cell_id),
            ..CellLookup::default()
        })?;

        cells
            .into_iter()
            .next()
            .ok_or(ContinuityError::CellNotFound { cell_id })
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use continuitydb_checkout::CheckoutRequest;
    use continuitydb_core::{
        Answerability, CellCost, CellPayload, Citation, CommitId, Confidence, Evidence, Scope,
        SemanticAnchor, SourceId, StateCell, StateCellId, TrustSignal, UtilityFeedback,
        ValidTimeRange,
    };
    use continuitydb_kernel::{CellLookup, CommitManifestLookup, KernelError, StorageKernel};
    use continuitydb_memory::MemoryKernel;

    use super::{ContinuityDb, ContinuityError};

    fn sample_cell(
        anchor: &str,
        confidence: f32,
        tokens: i64,
    ) -> Result<StateCell, Box<dyn std::error::Error>> {
        sample_cell_with_payload_day_and_confidence(anchor, anchor, 20, confidence, tokens)
    }

    fn sample_cell_with_payload_day_and_confidence(
        anchor: &str,
        payload: &str,
        valid_from_day: u32,
        confidence: f32,
        tokens: i64,
    ) -> Result<StateCell, Box<dyn std::error::Error>> {
        let valid_from = Utc
            .with_ymd_and_hms(2026, 5, valid_from_day, 0, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        StateCell::new(
            StateCellId::new(),
            vec![SemanticAnchor::new(anchor)],
            ValidTimeRange::new(valid_from, None)?,
            Scope::Project("continuitydb".to_string()),
            Answerability::new(vec!["what should the agent know?".to_string()])?,
            vec![Evidence {
                source: SourceId::new("test"),
                citation: Citation {
                    locator: format!("test://{anchor}"),
                },
                confidence: Confidence::new(confidence)?,
                trust: vec![TrustSignal::DirectObservation],
            }],
            CellPayload::Text(payload.to_string()),
            CellCost::new(tokens, 0)?,
        )
        .map_err(Into::into)
    }

    #[test]
    fn api_ingests_and_checkouts_cells() -> Result<(), Box<dyn std::error::Error>> {
        let committed_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let cell = sample_cell("project:continuitydb:api", 0.91, 12)?;
        let cell_id = cell.id;

        let returned_id = db.ingest_cell_at(cell, committed_at)?;
        let slice = db.checkout(CheckoutRequest {
            scope: Some(Scope::Project("continuitydb".to_string())),
            valid_at: Some(committed_at),
            system_at: Some(committed_at),
            commit_id: None,
            answerability_question: None,
            evidence_source: None,
            dependency_target: None,
            dependency_kind: None,
            minimum_confidence: Confidence::new(0.8)?,
            token_budget: 100,
        })?;

        assert_eq!(returned_id, cell_id);
        assert_eq!(slice.cells.len(), 1);
        assert_eq!(slice.cells[0].id, cell_id);
        assert_eq!(slice.cells[0].system_time.from(), committed_at);
        Ok(())
    }

    #[test]
    fn api_batch_ingest_returns_ordered_ids_and_shared_system_time(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let committed_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let first = sample_cell("project:continuitydb:batch-first", 0.91, 12)?;
        let second = sample_cell("project:continuitydb:batch-second", 0.83, 15)?;
        let expected_ids = vec![first.id, second.id];

        let returned_ids = db.ingest_cells_at(vec![first, second], committed_at)?;

        assert_eq!(returned_ids, expected_ids);
        let stored = db.kernel().lookup_cells(CellLookup::default())?;
        assert_eq!(
            stored.iter().map(|cell| cell.id).collect::<Vec<_>>(),
            expected_ids
        );
        assert!(stored
            .iter()
            .all(|cell| cell.system_time.from() == committed_at));
        Ok(())
    }

    #[test]
    fn api_batch_ingest_rejects_duplicates_without_partial_visibility(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let committed_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let cell = sample_cell("project:continuitydb:batch-duplicate", 0.91, 12)?;

        let result = db.ingest_cells_at(vec![cell.clone(), cell], committed_at);

        assert!(matches!(
            result,
            Err(ContinuityError::Kernel(KernelError::DuplicateCell))
        ));
        assert!(db.kernel().lookup_cells(CellLookup::default())?.is_empty());
        Ok(())
    }

    #[test]
    fn api_commit_id_batch_ingest_stamps_lookup_boundary() -> Result<(), Box<dyn std::error::Error>>
    {
        let committed_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let commit_id = CommitId::new();
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let first = sample_cell("project:continuitydb:commit-first", 0.91, 12)?;
        let second = sample_cell("project:continuitydb:commit-second", 0.83, 15)?;
        let expected_ids = vec![first.id, second.id];

        let returned_ids =
            db.ingest_cells_at_with_commit_id(vec![first, second], committed_at, commit_id)?;

        assert_eq!(returned_ids, expected_ids);
        let stored = db.kernel().lookup_cells(CellLookup {
            commit_id: Some(commit_id),
            ..CellLookup::default()
        })?;
        assert_eq!(
            stored.iter().map(|cell| cell.id).collect::<Vec<_>>(),
            expected_ids
        );
        assert!(stored.iter().all(|cell| cell.commit_id == commit_id));
        Ok(())
    }

    #[test]
    fn api_returns_commit_manifest_for_committed_batch() -> Result<(), Box<dyn std::error::Error>> {
        let committed_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let commit_id = CommitId::new();
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let first = sample_cell("project:continuitydb:manifest-first", 0.91, 12)?;
        let second = sample_cell("project:continuitydb:manifest-second", 0.83, 15)?;
        let expected_ids = vec![first.id, second.id];

        db.ingest_cells_at_with_commit_id(vec![first, second], committed_at, commit_id)?;

        let manifest = db
            .commit_manifest(commit_id)?
            .ok_or_else(|| std::io::Error::other("missing manifest"))?;
        assert_eq!(manifest.commit_id, commit_id);
        assert_eq!(manifest.committed_at, committed_at);
        assert_eq!(manifest.cell_ids, expected_ids);
        Ok(())
    }

    #[test]
    fn api_returns_none_for_unknown_commit_manifest() -> Result<(), Box<dyn std::error::Error>> {
        let db = ContinuityDb::new(MemoryKernel::default());

        assert!(db.commit_manifest(CommitId::new())?.is_none());
        Ok(())
    }

    #[test]
    fn api_returns_commit_manifests_in_kernel_order() -> Result<(), Box<dyn std::error::Error>> {
        let first_time = Utc
            .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let second_time = Utc
            .with_ymd_and_hms(2026, 5, 20, 12, 30, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let first_commit = CommitId::new();
        let second_commit = CommitId::new();
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let first = sample_cell("project:continuitydb:list-first", 0.91, 12)?;
        let second = sample_cell("project:continuitydb:list-second", 0.83, 15)?;

        db.ingest_cells_at_with_commit_id(vec![first], first_time, first_commit)?;
        db.ingest_cells_at_with_commit_id(vec![second], second_time, second_commit)?;

        let manifests = db.commit_manifests()?;
        assert_eq!(
            manifests
                .iter()
                .map(|manifest| manifest.commit_id)
                .collect::<Vec<_>>(),
            vec![first_commit, second_commit]
        );
        assert_eq!(manifests[0].committed_at, first_time);
        assert_eq!(manifests[1].committed_at, second_time);
        Ok(())
    }

    #[test]
    fn api_returns_commit_manifests_after_cursor_with_limit(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let first_time = Utc
            .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
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
        let mut db = ContinuityDb::new(MemoryKernel::default());
        db.ingest_cells_at_with_commit_id(
            vec![sample_cell(
                "project:continuitydb:api-cursor-first",
                0.91,
                12,
            )?],
            first_time,
            first_commit,
        )?;
        db.ingest_cells_at_with_commit_id(
            vec![sample_cell(
                "project:continuitydb:api-cursor-second",
                0.83,
                15,
            )?],
            second_time,
            second_commit,
        )?;
        db.ingest_cells_at_with_commit_id(
            vec![sample_cell(
                "project:continuitydb:api-cursor-third",
                0.77,
                18,
            )?],
            third_time,
            third_commit,
        )?;

        let manifests = db.commit_manifests_matching(CommitManifestLookup {
            after: Some(first_commit),
            limit: Some(1),
        })?;

        assert_eq!(manifests.len(), 1);
        assert_eq!(manifests[0].commit_id, second_commit);
        Ok(())
    }

    #[test]
    fn api_audits_cell_by_id_with_structured_evidence() -> Result<(), Box<dyn std::error::Error>> {
        let committed_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let cell = sample_cell("project:continuitydb:audit-api", 0.91, 12)?;
        let cell_id = db.ingest_cell_at(cell, committed_at)?;

        let trace = db.audit_cell(cell_id)?;

        assert_eq!(trace.cell_id, cell_id);
        assert_eq!(trace.evidence.len(), 1);
        assert_eq!(trace.evidence[0].source, "test");
        assert_eq!(
            trace.evidence[0].locator,
            "test://project:continuitydb:audit-api"
        );
        assert_eq!(trace.evidence[0].confidence, Confidence::new(0.91)?);
        assert_eq!(
            trace.evidence[0].trust,
            vec![TrustSignal::DirectObservation]
        );
        Ok(())
    }

    #[test]
    fn api_audit_cell_reports_missing_id() {
        let db = ContinuityDb::new(MemoryKernel::default());
        let missing_id = StateCellId::new();

        let result = db.audit_cell(missing_id);

        assert!(matches!(
            result,
            Err(ContinuityError::CellNotFound { cell_id }) if cell_id == missing_id
        ));
    }

    #[test]
    fn api_records_utility_feedback_as_successor_cell() -> Result<(), Box<dyn std::error::Error>> {
        let initial_commit = Utc
            .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let feedback_commit = Utc
            .with_ymd_and_hms(2026, 5, 20, 12, 30, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let cell = sample_cell("project:continuitydb:feedback-api", 0.91, 12)?;
        let original_feedback = UtilityFeedback::default();
        let feedback = UtilityFeedback::new(
            Confidence::new(0.9)?,
            Confidence::new(0.8)?,
            Confidence::new(0.7)?,
        );
        let original_id = db.ingest_cell_at(cell, initial_commit)?;

        let successor_id = db.record_utility_feedback_at(original_id, feedback, feedback_commit)?;

        let original = db
            .kernel()
            .lookup_cells(CellLookup {
                cell_id: Some(original_id),
                ..CellLookup::default()
            })?
            .into_iter()
            .next()
            .ok_or_else(|| std::io::Error::other("missing original cell"))?;
        let successor = db
            .kernel()
            .lookup_cells(CellLookup {
                cell_id: Some(successor_id),
                ..CellLookup::default()
            })?
            .into_iter()
            .next()
            .ok_or_else(|| std::io::Error::other("missing successor cell"))?;

        assert_ne!(successor_id, original_id);
        assert_eq!(db.audit_cell(successor_id)?.cell_id, successor_id);
        assert_eq!(original.utility_feedback, original_feedback);
        assert_eq!(successor.utility_feedback, feedback);
        assert_eq!(successor.system_time.from(), feedback_commit);
        Ok(())
    }

    #[test]
    fn api_record_utility_feedback_reports_missing_id() -> Result<(), Box<dyn std::error::Error>> {
        let feedback_commit = Utc
            .with_ymd_and_hms(2026, 5, 20, 12, 30, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let missing_id = StateCellId::new();
        let feedback = UtilityFeedback::new(
            Confidence::new(0.9)?,
            Confidence::new(0.8)?,
            Confidence::new(0.7)?,
        );

        let result = db.record_utility_feedback_at(missing_id, feedback, feedback_commit);

        assert!(matches!(
            result,
            Err(ContinuityError::CellNotFound { cell_id }) if cell_id == missing_id
        ));
        Ok(())
    }

    #[test]
    fn api_detects_conflict_between_stored_cells() -> Result<(), Box<dyn std::error::Error>> {
        let committed_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let left = sample_cell_with_payload_day_and_confidence(
            "project:continuitydb:conflict",
            "release is ready",
            20,
            0.95,
            12,
        )?;
        let right = sample_cell_with_payload_day_and_confidence(
            "project:continuitydb:conflict",
            "release is blocked",
            20,
            0.60,
            12,
        )?;
        let left_id = db.ingest_cell_at(left, committed_at)?;
        let right_id = db.ingest_cell_at(right, committed_at)?;

        let conflict = db
            .detect_conflict(left_id, right_id)?
            .ok_or_else(|| std::io::Error::other("missing conflict"))?;

        assert_eq!(conflict.left, left_id);
        assert_eq!(conflict.right, right_id);
        assert_eq!(
            conflict.kind,
            continuitydb_revision::CellConflictKind::PayloadMismatch
        );
        Ok(())
    }

    #[test]
    fn api_recommends_conflict_resolution_between_stored_cells(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let committed_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let left = sample_cell_with_payload_day_and_confidence(
            "project:continuitydb:recommendation",
            "release is ready",
            20,
            0.95,
            12,
        )?;
        let right = sample_cell_with_payload_day_and_confidence(
            "project:continuitydb:recommendation",
            "release is blocked",
            20,
            0.60,
            12,
        )?;
        let left_id = db.ingest_cell_at(left, committed_at)?;
        let right_id = db.ingest_cell_at(right, committed_at)?;

        let recommendation = db
            .recommend_conflict_resolution(left_id, right_id)?
            .ok_or_else(|| std::io::Error::other("missing recommendation"))?;

        assert_eq!(
            recommendation.kind,
            continuitydb_revision::ConflictResolutionKind::CandidateSupersession
        );
        assert_eq!(recommendation.winner, Some(left_id));
        assert_eq!(recommendation.loser, Some(right_id));
        Ok(())
    }

    #[test]
    fn api_conflict_analysis_returns_none_for_non_conflicting_cells(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let committed_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let left = sample_cell_with_payload_day_and_confidence(
            "project:continuitydb:left",
            "release is ready",
            20,
            0.95,
            12,
        )?;
        let right = sample_cell_with_payload_day_and_confidence(
            "project:continuitydb:right",
            "release is blocked",
            20,
            0.60,
            12,
        )?;
        let left_id = db.ingest_cell_at(left, committed_at)?;
        let right_id = db.ingest_cell_at(right, committed_at)?;

        assert!(db.detect_conflict(left_id, right_id)?.is_none());
        assert!(db
            .recommend_conflict_resolution(left_id, right_id)?
            .is_none());
        Ok(())
    }

    #[test]
    fn api_conflict_analysis_reports_missing_id() -> Result<(), Box<dyn std::error::Error>> {
        let committed_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let left = sample_cell_with_payload_day_and_confidence(
            "project:continuitydb:left",
            "release is ready",
            20,
            0.95,
            12,
        )?;
        let left_id = db.ingest_cell_at(left, committed_at)?;
        let missing_id = StateCellId::new();

        let conflict = db.detect_conflict(left_id, missing_id);
        let recommendation = db.recommend_conflict_resolution(left_id, missing_id);

        assert!(matches!(
            conflict,
            Err(ContinuityError::CellNotFound { cell_id }) if cell_id == missing_id
        ));
        assert!(matches!(
            recommendation,
            Err(ContinuityError::CellNotFound { cell_id }) if cell_id == missing_id
        ));
        Ok(())
    }

    #[test]
    fn api_detects_batch_conflicts_for_stored_cell_set() -> Result<(), Box<dyn std::error::Error>> {
        let committed_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let left = sample_cell_with_payload_day_and_confidence(
            "project:continuitydb:batch-conflict",
            "release is ready",
            20,
            0.95,
            12,
        )?;
        let right = sample_cell_with_payload_day_and_confidence(
            "project:continuitydb:batch-conflict",
            "release is blocked",
            20,
            0.60,
            12,
        )?;
        let unrelated = sample_cell_with_payload_day_and_confidence(
            "project:continuitydb:unrelated",
            "unrelated state",
            20,
            0.90,
            12,
        )?;
        let left_id = db.ingest_cell_at(left, committed_at)?;
        let right_id = db.ingest_cell_at(right, committed_at)?;
        let unrelated_id = db.ingest_cell_at(unrelated, committed_at)?;

        let scan = db.detect_conflicts([left_id, right_id, unrelated_id])?;

        assert_eq!(scan.conflicts.len(), 1);
        assert_eq!(scan.conflicts[0].left, left_id);
        assert_eq!(scan.conflicts[0].right, right_id);
        Ok(())
    }

    #[test]
    fn api_recommends_batch_conflict_resolutions_for_stored_cell_set(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let committed_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let left = sample_cell_with_payload_day_and_confidence(
            "project:continuitydb:batch-recommendation",
            "release is ready",
            20,
            0.95,
            12,
        )?;
        let right = sample_cell_with_payload_day_and_confidence(
            "project:continuitydb:batch-recommendation",
            "release is blocked",
            20,
            0.60,
            12,
        )?;
        let unrelated = sample_cell_with_payload_day_and_confidence(
            "project:continuitydb:unrelated",
            "unrelated state",
            20,
            0.90,
            12,
        )?;
        let left_id = db.ingest_cell_at(left, committed_at)?;
        let right_id = db.ingest_cell_at(right, committed_at)?;
        let unrelated_id = db.ingest_cell_at(unrelated, committed_at)?;

        let scan = db.recommend_conflict_resolutions([left_id, right_id, unrelated_id])?;

        assert_eq!(scan.recommendations.len(), 1);
        assert_eq!(
            scan.recommendations[0].kind,
            continuitydb_revision::ConflictResolutionKind::CandidateSupersession
        );
        assert_eq!(scan.recommendations[0].winner, Some(left_id));
        assert_eq!(scan.recommendations[0].loser, Some(right_id));
        Ok(())
    }

    #[test]
    fn api_batch_conflict_analysis_allows_empty_and_singleton_inputs(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let committed_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let cell = sample_cell_with_payload_day_and_confidence(
            "project:continuitydb:singleton",
            "release is ready",
            20,
            0.95,
            12,
        )?;
        let cell_id = db.ingest_cell_at(cell, committed_at)?;

        assert!(db.detect_conflicts([])?.conflicts.is_empty());
        assert!(db
            .recommend_conflict_resolutions([])?
            .recommendations
            .is_empty());
        assert!(db.detect_conflicts([cell_id])?.conflicts.is_empty());
        assert!(db
            .recommend_conflict_resolutions([cell_id])?
            .recommendations
            .is_empty());
        Ok(())
    }

    #[test]
    fn api_batch_conflict_analysis_reports_missing_id() -> Result<(), Box<dyn std::error::Error>> {
        let committed_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let cell = sample_cell_with_payload_day_and_confidence(
            "project:continuitydb:stored",
            "release is ready",
            20,
            0.95,
            12,
        )?;
        let stored_id = db.ingest_cell_at(cell, committed_at)?;
        let missing_id = StateCellId::new();

        let conflicts = db.detect_conflicts([stored_id, missing_id]);
        let recommendations = db.recommend_conflict_resolutions([stored_id, missing_id]);

        assert!(matches!(
            conflicts,
            Err(ContinuityError::CellNotFound { cell_id }) if cell_id == missing_id
        ));
        assert!(matches!(
            recommendations,
            Err(ContinuityError::CellNotFound { cell_id }) if cell_id == missing_id
        ));
        Ok(())
    }
}
