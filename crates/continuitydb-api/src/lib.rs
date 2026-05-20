//! Native embeddable operation API for ContinuityDB.

use chrono::{DateTime, Utc};
use continuitydb_checkout::{
    audit, checkout, AuditTrace, CheckoutError, CheckoutRequest, CheckoutSlice,
};
#[cfg(feature = "steward")]
use continuitydb_core::{
    ActivationState, Answerability, CellCost, CellDependency, CellDependencyKind, CellPayload,
    Citation, Confidence, Evidence, Scope, SemanticAnchor, SourceId, TrustSignal, ValidTimeRange,
};
use continuitydb_core::{
    CommitId, CommitManifest, CoreError, StateCell, StateCellId, UtilityFeedback,
};
use continuitydb_kernel::{
    CellLookup, CommitManifestLookup, FileKernel, FileKernelHealth, FileKernelStatus,
    KernelCapabilities, KernelError, KernelRequirements, StorageKernel,
};
use continuitydb_query::{
    decode_query_json, parse_query_text, CheckoutQuery, ContinuityQuery, QueryEnvelopeError,
    QueryError, QueryTextError,
};
use continuitydb_revision::{
    detect_cell_conflict, recommend_conflict_resolution, recommend_conflict_resolutions,
    revise_utility_feedback, scan_cell_conflicts, CellConflict, CellConflictScan,
    ConflictResolutionRecommendation, ConflictResolutionScan,
};
#[cfg(feature = "steward")]
use continuitydb_revision::{
    revise_activation_state, revise_answerability, revise_evidence_confidence,
};
#[cfg(feature = "steward")]
use continuitydb_steward::{
    BorrowedKernelProposalStore, ConflictResolutionSteward, FrontierSubscriptionRunner,
    FrontierSubscriptionStore, FrontierWatchEvent, ProposalAuditRecord, ProposalId,
    ProposalOutcome, ProposalPolicy, StewardAction, StewardError, StewardProposal,
    StoredProposalLedger,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, fs, path::Path};
use thiserror::Error;

/// Wire-format marker for JSON commit export envelopes.
pub const COMMIT_EXPORT_FORMAT: &str = "continuitydb.commit_export";
/// Supported JSON commit export envelope version.
pub const COMMIT_EXPORT_FORMAT_VERSION: u32 = 1;

/// Errors produced by the native ContinuityDB operation API.
#[derive(Debug, Error, PartialEq)]
pub enum ContinuityError {
    /// Storage kernel failure.
    #[error(transparent)]
    Kernel(#[from] KernelError),
    /// Checkout operation failure.
    #[error(transparent)]
    Checkout(#[from] CheckoutError),
    /// Query compilation failure.
    #[error(transparent)]
    Query(#[from] QueryError),
    /// Query envelope decoding or validation failure.
    #[error(transparent)]
    QueryEnvelope(#[from] QueryEnvelopeError),
    /// Query text parsing failure.
    #[error(transparent)]
    QueryText(#[from] QueryTextError),
    /// Steward proposal audit failure.
    #[cfg(feature = "steward")]
    #[error(transparent)]
    Steward(#[from] StewardError),
    /// Accepted Steward proposal action is not supported by this application API.
    #[cfg(feature = "steward")]
    #[error("steward proposal action is not supported by this application API")]
    UnsupportedStewardProposalAction,
    /// Core semantic validation failure.
    #[error(transparent)]
    Core(#[from] CoreError),
    /// Raw typed query JSON could not be decoded.
    #[error("query JSON is invalid")]
    QueryJson,
    /// Query file could not be read.
    #[error("query file I/O failed")]
    QueryFileIo,
    /// Requested StateCell was not found in the backing kernel.
    #[error("state cell not found")]
    CellNotFound {
        /// Missing cell identifier.
        cell_id: StateCellId,
    },
    /// Commit export batch failed deterministic validation.
    #[error("commit export batch is invalid for commit {commit_id:?}")]
    InvalidCommitExport {
        /// Commit whose export slice failed validation.
        commit_id: CommitId,
    },
    /// Commit export envelope has an unsupported format or version.
    #[error("commit export envelope is invalid")]
    InvalidCommitExportEnvelope,
    /// Commit export envelope JSON could not be encoded or decoded.
    #[error("commit export envelope JSON is invalid")]
    CommitExportJson,
    /// Commit export envelope file could not be read or written.
    #[error("commit export envelope file I/O failed")]
    CommitExportFileIo,
    /// Backing kernel does not satisfy required storage guarantees.
    #[error("storage kernel requirements are not met")]
    KernelRequirementsNotMet {
        /// Required storage guarantees.
        required: KernelRequirements,
        /// Actual backing kernel guarantees.
        actual: KernelCapabilities,
    },
    /// File store is readable but should be compacted before use under the requested policy.
    #[error("file store compaction is recommended")]
    FileStoreCompactionRecommended {
        /// Health report that explains why the store is not canonical.
        health: FileKernelHealth,
    },
}

/// Native embeddable ContinuityDB operation boundary.
#[derive(Clone, Debug)]
pub struct ContinuityDb<K> {
    kernel: K,
}

/// Materialized cells for one database commit boundary.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CommitSlice {
    /// Commit manifest that defines the boundary and cell order.
    pub manifest: CommitManifest,
    /// StateCells written by the commit, in manifest order.
    pub cells: Vec<StateCell>,
}

/// Cursor-selected commit slices ready for backup, sync, or replay export.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CommitExportBatch {
    /// Exported commit slices in database visibility order.
    pub slices: Vec<CommitSlice>,
    /// Cursor to use as `CommitManifestLookup.after` for the next export batch.
    pub next_after: Option<CommitId>,
}

/// Result of deterministic conflict-resolution stewardship recorded through the native API.
#[cfg(feature = "steward")]
pub struct StewardConflictAudit {
    /// Deterministic conflict-resolution scan for the requested cells.
    pub scan: ConflictResolutionScan,
    /// Steward proposals emitted from the scan.
    pub proposals: Vec<StewardProposal>,
    /// Policy-evaluated proposal audit records appended to the backing kernel.
    pub records: Vec<ProposalAuditRecord>,
}

/// Result of subscribed frontier/watch stewardship recorded through the native API.
#[cfg(feature = "steward")]
pub struct StewardFrontierAudit {
    /// Steward proposals emitted from subscribed frontier watch events.
    pub proposals: Vec<StewardProposal>,
    /// Policy-evaluated proposal audit records appended to the backing kernel.
    pub records: Vec<ProposalAuditRecord>,
}

/// Summary of a commit export envelope written to a file.
#[derive(Clone, Debug, PartialEq)]
pub struct CommitExportFileSummary {
    /// Number of commit slices exported.
    pub exported_commits: usize,
    /// Cursor to use as `CommitManifestLookup.after` for the next export batch.
    pub next_after: Option<CommitId>,
}

/// Summary of a non-mutating commit import validation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommitImportValidation {
    /// Number of commit slices that would be imported.
    pub valid_commits: usize,
}

/// Summary of a successful commit import.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommitImportSummary {
    /// Number of commit slices imported.
    pub imported_commits: usize,
    /// Cursor from the imported export batch.
    pub next_after: Option<CommitId>,
}

/// Summary of a conditional file-store compaction attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FileCompactionSummary {
    /// Whether the backing store was rewritten.
    pub compacted: bool,
    /// File-store health before the maintenance decision.
    pub before: FileKernelHealth,
    /// File-store health after the maintenance decision.
    pub after: FileKernelHealth,
}

/// Versioned JSON envelope for portable commit export batches.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CommitExportEnvelope {
    /// Wire-format marker.
    pub format: String,
    /// Wire-format version.
    pub version: u32,
    /// Exported commit batch.
    pub batch: CommitExportBatch,
}

impl CommitExportEnvelope {
    /// Wraps a commit export batch in the current JSON envelope.
    pub fn new(batch: CommitExportBatch) -> Self {
        Self {
            format: COMMIT_EXPORT_FORMAT.to_string(),
            version: COMMIT_EXPORT_FORMAT_VERSION,
            batch,
        }
    }

    /// Validates the envelope format and version.
    pub fn validate(&self) -> Result<(), ContinuityError> {
        if self.format == COMMIT_EXPORT_FORMAT && self.version == COMMIT_EXPORT_FORMAT_VERSION {
            Ok(())
        } else {
            Err(ContinuityError::InvalidCommitExportEnvelope)
        }
    }
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

    /// Encodes a commit export batch as a versioned JSON envelope.
    pub fn encode_commit_export_json(batch: CommitExportBatch) -> Result<Vec<u8>, ContinuityError> {
        serde_json::to_vec(&CommitExportEnvelope::new(batch))
            .map_err(|_error| ContinuityError::CommitExportJson)
    }

    /// Decodes a versioned JSON commit export envelope.
    pub fn decode_commit_export_json(bytes: &[u8]) -> Result<CommitExportBatch, ContinuityError> {
        let envelope = serde_json::from_slice::<CommitExportEnvelope>(bytes)
            .map_err(|_error| ContinuityError::CommitExportJson)?;
        envelope.validate()?;
        Ok(envelope.batch)
    }
}

impl<K: StorageKernel> ContinuityDb<K> {
    /// Returns the storage guarantees exposed by the backing kernel.
    pub fn kernel_capabilities(&self) -> KernelCapabilities {
        self.kernel.capabilities()
    }

    /// Returns true when the backing kernel satisfies the requested storage guarantees.
    pub fn kernel_satisfies(&self, requirements: KernelRequirements) -> bool {
        self.kernel_capabilities().satisfies(requirements)
    }

    /// Fails when the backing kernel does not satisfy the requested storage guarantees.
    pub fn ensure_kernel_requirements(
        &self,
        required: KernelRequirements,
    ) -> Result<(), ContinuityError> {
        let actual = self.kernel_capabilities();
        if actual.satisfies(required) {
            Ok(())
        } else {
            Err(ContinuityError::KernelRequirementsNotMet { required, actual })
        }
    }

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

    /// Materializes a deterministic continuity slice from a typed checkout query.
    pub fn checkout_query(&self, query: CheckoutQuery) -> Result<CheckoutSlice, ContinuityError> {
        self.checkout(query.compile_checkout()?)
    }

    /// Materializes a deterministic continuity slice from a top-level typed query.
    pub fn checkout_continuity_query(
        &self,
        query: ContinuityQuery,
    ) -> Result<CheckoutSlice, ContinuityError> {
        self.checkout(query.compile_checkout()?)
    }

    /// Materializes a deterministic continuity slice from a versioned typed query JSON envelope.
    pub fn checkout_query_json(&self, bytes: &[u8]) -> Result<CheckoutSlice, ContinuityError> {
        self.checkout_continuity_query(decode_query_json(bytes)?)
    }

    /// Materializes a deterministic continuity slice from strict text query syntax.
    pub fn checkout_query_text(&self, input: &str) -> Result<CheckoutSlice, ContinuityError> {
        self.checkout_continuity_query(parse_query_text(input)?)
    }

    /// Materializes a deterministic continuity slice from a saved typed query file.
    pub fn checkout_query_file<P: AsRef<Path>>(
        &self,
        query_path: P,
    ) -> Result<CheckoutSlice, ContinuityError> {
        let encoded = fs::read(query_path).map_err(|_error| ContinuityError::QueryFileIo)?;
        self.checkout_continuity_query(decode_query_file(&encoded)?)
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

    /// Returns StateCells written by a database commit in manifest order.
    pub fn commit_cells(&self, commit_id: CommitId) -> Result<Vec<StateCell>, ContinuityError> {
        let manifest = self
            .commit_manifest(commit_id)?
            .ok_or(ContinuityError::Kernel(KernelError::CommitNotFound))?;
        self.lookup_cells_in_order(manifest.cell_ids)
    }

    /// Returns commit manifests and their StateCells matching deterministic listing constraints.
    pub fn commit_slices(
        &self,
        lookup: CommitManifestLookup,
    ) -> Result<Vec<CommitSlice>, ContinuityError> {
        self.commit_manifests_matching(lookup)?
            .into_iter()
            .map(|manifest| {
                let cells = self.lookup_cells_in_order(manifest.cell_ids.clone())?;
                Ok(CommitSlice { manifest, cells })
            })
            .collect()
    }

    /// Returns a cursor-selected commit export batch for backup, sync, and replay flows.
    pub fn export_commits(
        &self,
        lookup: CommitManifestLookup,
    ) -> Result<CommitExportBatch, ContinuityError> {
        let slices = self.commit_slices(lookup)?;
        let next_after = slices.last().map(|slice| slice.manifest.commit_id);
        Ok(CommitExportBatch { slices, next_after })
    }

    /// Copies a cursor-selected commit page from another open database into this database.
    pub fn copy_commits_from<S: StorageKernel>(
        &mut self,
        source: &ContinuityDb<S>,
        lookup: CommitManifestLookup,
    ) -> Result<CommitImportSummary, ContinuityError> {
        let batch = source.export_commits(lookup)?;
        self.import_commit_batch_with_summary(batch)
    }

    /// Imports a validated commit export batch into the backing kernel.
    pub fn import_commit_batch(
        &mut self,
        batch: CommitExportBatch,
    ) -> Result<usize, ContinuityError> {
        let summary = self.import_commit_batch_with_summary(batch)?;
        Ok(summary.imported_commits)
    }

    /// Imports a validated commit export batch into the backing kernel and returns cursor metadata.
    pub fn import_commit_batch_with_summary(
        &mut self,
        batch: CommitExportBatch,
    ) -> Result<CommitImportSummary, ContinuityError> {
        self.validate_commit_import(&batch)?;
        let imported = batch.slices.len();
        let next_after = batch.next_after;
        for slice in batch.slices {
            self.kernel.append_cells_at_with_commit_id(
                slice.cells,
                slice.manifest.committed_at,
                slice.manifest.commit_id,
            )?;
        }
        Ok(CommitImportSummary {
            imported_commits: imported,
            next_after,
        })
    }

    /// Validates a commit export batch without mutating the backing kernel.
    pub fn validate_commit_import(
        &self,
        batch: &CommitExportBatch,
    ) -> Result<CommitImportValidation, ContinuityError> {
        self.validate_commit_export_batch(batch)?;
        Ok(CommitImportValidation {
            valid_commits: batch.slices.len(),
        })
    }

    /// Records a Steward proposal and deterministic policy decision as audit StateCell evidence.
    #[cfg(feature = "steward")]
    pub fn record_steward_proposal(
        &mut self,
        proposal: StewardProposal,
        policy: &ProposalPolicy,
        decided_at: DateTime<Utc>,
    ) -> Result<ProposalAuditRecord, ContinuityError> {
        let decision = policy.evaluate(&proposal, decided_at);
        let record = ProposalAuditRecord::new(proposal.clone(), decision.clone())?;
        let store = BorrowedKernelProposalStore::new(&mut self.kernel);
        let mut ledger = StoredProposalLedger::new(store);
        ledger.record(proposal, decision)?;
        Ok(record)
    }

    /// Returns all Steward proposal audit records stored in the backing kernel.
    #[cfg(feature = "steward")]
    pub fn steward_proposal_records(
        &mut self,
    ) -> Result<Vec<ProposalAuditRecord>, ContinuityError> {
        let store = BorrowedKernelProposalStore::new(&mut self.kernel);
        let ledger = StoredProposalLedger::new(store);
        Ok(ledger.records()?)
    }

    /// Returns one Steward proposal audit record by proposal ID.
    #[cfg(feature = "steward")]
    pub fn steward_proposal_record(
        &mut self,
        proposal_id: ProposalId,
    ) -> Result<Option<ProposalAuditRecord>, ContinuityError> {
        let store = BorrowedKernelProposalStore::new(&mut self.kernel);
        let ledger = StoredProposalLedger::new(store);
        Ok(ledger.record_by_id(proposal_id)?)
    }

    /// Runs subscribed frontier/watch stewardship and records proposal audit StateCells.
    #[cfg(feature = "steward")]
    pub fn audit_frontier_watch_with_steward<S>(
        &mut self,
        runner: &FrontierSubscriptionRunner<S>,
        events: Vec<FrontierWatchEvent>,
        policy: &ProposalPolicy,
        decided_at: DateTime<Utc>,
    ) -> Result<StewardFrontierAudit, ContinuityError>
    where
        S: FrontierSubscriptionStore,
    {
        let proposals = runner.propose_subscribed(events, decided_at)?;
        let mut records = Vec::with_capacity(proposals.len());
        let store = BorrowedKernelProposalStore::new(&mut self.kernel);
        let mut ledger = StoredProposalLedger::new(store);

        for proposal in proposals.iter().cloned() {
            let decision = policy.evaluate(&proposal, decided_at);
            let record = ProposalAuditRecord::new(proposal.clone(), decision.clone())?;
            ledger.record(proposal, decision)?;
            records.push(record);
        }

        Ok(StewardFrontierAudit { proposals, records })
    }

    /// Applies an accepted MarkFrontier Steward proposal as an append-only successor StateCell.
    #[cfg(feature = "steward")]
    pub fn apply_accepted_mark_frontier_proposal_at(
        &mut self,
        record: &ProposalAuditRecord,
        committed_at: DateTime<Utc>,
    ) -> Result<Option<StateCellId>, ContinuityError> {
        if record.decision().outcome() == ProposalOutcome::Rejected {
            return Ok(None);
        }

        let cell_id = match record.proposal().action() {
            StewardAction::MarkFrontier { cell_id } => *cell_id,
            _ => return Err(ContinuityError::UnsupportedStewardProposalAction),
        };

        let previous = self.lookup_one_cell(cell_id)?;
        let revision = revise_activation_state(&previous, ActivationState::Frontier);
        let successor_id = revision.cell.id;
        self.kernel.append_cell_at(revision.cell, committed_at)?;
        Ok(Some(successor_id))
    }

    /// Applies an accepted LabelAnswerability Steward proposal as an append-only successor StateCell.
    #[cfg(feature = "steward")]
    pub fn apply_accepted_label_answerability_proposal_at(
        &mut self,
        record: &ProposalAuditRecord,
        committed_at: DateTime<Utc>,
    ) -> Result<Option<StateCellId>, ContinuityError> {
        if record.decision().outcome() == ProposalOutcome::Rejected {
            return Ok(None);
        }

        let (cell_id, questions) = match record.proposal().action() {
            StewardAction::LabelAnswerability { cell_id, questions } => (*cell_id, questions),
            _ => return Err(ContinuityError::UnsupportedStewardProposalAction),
        };

        let answerability = Answerability::new(questions.clone())?;
        let previous = self.lookup_one_cell(cell_id)?;
        let revision = revise_answerability(&previous, answerability);
        let successor_id = revision.cell.id;
        self.kernel.append_cell_at(revision.cell, committed_at)?;
        Ok(Some(successor_id))
    }

    /// Applies an accepted AdjustConfidence Steward proposal as an append-only successor StateCell.
    #[cfg(feature = "steward")]
    pub fn apply_accepted_adjust_confidence_proposal_at(
        &mut self,
        record: &ProposalAuditRecord,
        committed_at: DateTime<Utc>,
    ) -> Result<Option<StateCellId>, ContinuityError> {
        if record.decision().outcome() == ProposalOutcome::Rejected {
            return Ok(None);
        }

        let (cell_id, proposed_confidence) = match record.proposal().action() {
            StewardAction::AdjustConfidence {
                cell_id,
                proposed_confidence,
            } => (*cell_id, *proposed_confidence),
            _ => return Err(ContinuityError::UnsupportedStewardProposalAction),
        };

        let confidence = Confidence::new(proposed_confidence)?;
        let previous = self.lookup_one_cell(cell_id)?;
        let revision = revise_evidence_confidence(&previous, confidence);
        let successor_id = revision.cell.id;
        self.kernel.append_cell_at(revision.cell, committed_at)?;
        Ok(Some(successor_id))
    }

    /// Applies an accepted CreateCellDraft Steward proposal as an append-only StateCell.
    #[cfg(feature = "steward")]
    pub fn apply_accepted_create_cell_draft_proposal_at(
        &mut self,
        record: &ProposalAuditRecord,
        committed_at: DateTime<Utc>,
    ) -> Result<Option<StateCellId>, ContinuityError> {
        if record.decision().outcome() == ProposalOutcome::Rejected {
            return Ok(None);
        }

        let (anchors, payload_text) = match record.proposal().action() {
            StewardAction::CreateCellDraft {
                anchors,
                payload_text,
            } => (anchors.clone(), payload_text.clone()),
            _ => return Err(ContinuityError::UnsupportedStewardProposalAction),
        };

        let derived_confidence = Confidence::new(1.0)?;
        let cell = StateCell::new(
            StateCellId::new(),
            anchors,
            ValidTimeRange::new(committed_at, None)?,
            Scope::Project("continuitydb-steward".to_string()),
            Answerability::new(vec!["what StateCell did the Steward draft?".to_string()])?,
            record
                .proposal()
                .citations()
                .iter()
                .map(|citation| Evidence {
                    source: SourceId::new("continuitydb-steward"),
                    citation: Citation {
                        locator: citation.clone(),
                    },
                    confidence: derived_confidence,
                    trust: vec![TrustSignal::Derived],
                })
                .collect(),
            CellPayload::Text(payload_text.clone()),
            CellCost::new(payload_text.split_whitespace().count() as i64, 0)?,
        )?;

        let cell_id = cell.id;
        self.kernel.append_cell_at(cell, committed_at)?;
        Ok(Some(cell_id))
    }

    /// Applies an accepted Steward proposal by dispatching to the action-specific application API.
    #[cfg(feature = "steward")]
    pub fn apply_accepted_steward_proposal_at(
        &mut self,
        record: &ProposalAuditRecord,
        committed_at: DateTime<Utc>,
    ) -> Result<Option<StateCellId>, ContinuityError> {
        match record.proposal().action() {
            StewardAction::CreateCellDraft { .. } => {
                self.apply_accepted_create_cell_draft_proposal_at(record, committed_at)
            }
            StewardAction::LinkRevision { .. } => {
                self.apply_accepted_link_revision_proposal_at(record, committed_at)
            }
            StewardAction::AdjustConfidence { .. } => {
                self.apply_accepted_adjust_confidence_proposal_at(record, committed_at)
            }
            StewardAction::LabelAnswerability { .. } => {
                self.apply_accepted_label_answerability_proposal_at(record, committed_at)
            }
            StewardAction::MarkFrontier { .. } => {
                self.apply_accepted_mark_frontier_proposal_at(record, committed_at)
            }
            StewardAction::RequestVerification { .. } => {
                self.apply_accepted_request_verification_proposal_at(record, committed_at)
            }
        }
    }

    /// Applies an accepted RequestVerification Steward proposal as an operational work StateCell.
    #[cfg(feature = "steward")]
    pub fn apply_accepted_request_verification_proposal_at(
        &mut self,
        record: &ProposalAuditRecord,
        committed_at: DateTime<Utc>,
    ) -> Result<Option<StateCellId>, ContinuityError> {
        if record.decision().outcome() == ProposalOutcome::Rejected {
            return Ok(None);
        }

        let (target_cell_id, request) = match record.proposal().action() {
            StewardAction::RequestVerification { cell_id, request } => (*cell_id, request),
            _ => return Err(ContinuityError::UnsupportedStewardProposalAction),
        };

        if let Some(cell_id) = target_cell_id {
            self.lookup_one_cell(cell_id)?;
        }

        let payload =
            serde_json::to_value(record).map_err(|_error| StewardError::ProposalStoreCorrupt)?;
        let target_anchor =
            target_cell_id.map_or_else(|| "general".to_string(), |cell_id| cell_id.to_string());
        let derived_confidence = Confidence::new(1.0)?;
        let mut cell = StateCell::new(
            StateCellId::new(),
            vec![
                SemanticAnchor::new("continuitydb:steward:verification-request"),
                SemanticAnchor::new(format!(
                    "continuitydb:steward:verification-request:{target_anchor}"
                )),
            ],
            ValidTimeRange::new(committed_at, None)?,
            Scope::Project("continuitydb-steward".to_string()),
            Answerability::new(vec![
                "what verification did the Steward request?".to_string()
            ])?,
            record
                .proposal()
                .citations()
                .iter()
                .map(|citation| Evidence {
                    source: SourceId::new("continuitydb-steward"),
                    citation: Citation {
                        locator: citation.clone(),
                    },
                    confidence: derived_confidence,
                    trust: vec![TrustSignal::Derived],
                })
                .collect(),
            CellPayload::Json(payload),
            CellCost::new(request.split_whitespace().count() as i64, 0)?,
        )?;

        if let Some(cell_id) = target_cell_id {
            cell.dependencies.push(CellDependency::new(
                cell_id,
                CellDependencyKind::DependsOn,
                "verification request targets this StateCell",
            ));
        }

        let work_cell_id = cell.id;
        self.kernel.append_cell_at(cell, committed_at)?;
        Ok(Some(work_cell_id))
    }

    /// Applies an accepted LinkRevision Steward proposal as an operational link StateCell.
    #[cfg(feature = "steward")]
    pub fn apply_accepted_link_revision_proposal_at(
        &mut self,
        record: &ProposalAuditRecord,
        committed_at: DateTime<Utc>,
    ) -> Result<Option<StateCellId>, ContinuityError> {
        if record.decision().outcome() == ProposalOutcome::Rejected {
            return Ok(None);
        }

        let (source, kind, target) = match record.proposal().action() {
            StewardAction::LinkRevision {
                source,
                kind,
                target,
            } => (*source, *kind, *target),
            _ => return Err(ContinuityError::UnsupportedStewardProposalAction),
        };

        self.lookup_one_cell(source)?;
        self.lookup_one_cell(target)?;

        let payload =
            serde_json::to_value(record).map_err(|_error| StewardError::ProposalStoreCorrupt)?;
        let derived_confidence = Confidence::new(1.0)?;
        let mut cell = StateCell::new(
            StateCellId::new(),
            vec![
                SemanticAnchor::new("continuitydb:steward:revision-link"),
                SemanticAnchor::new(format!(
                    "continuitydb:steward:revision-link:{source}:{kind:?}:{target}"
                )),
            ],
            ValidTimeRange::new(committed_at, None)?,
            Scope::Project("continuitydb-steward".to_string()),
            Answerability::new(vec![
                "what revision link did the Steward propose?".to_string()
            ])?,
            record
                .proposal()
                .citations()
                .iter()
                .map(|citation| Evidence {
                    source: SourceId::new("continuitydb-steward"),
                    citation: Citation {
                        locator: citation.clone(),
                    },
                    confidence: derived_confidence,
                    trust: vec![TrustSignal::Derived],
                })
                .collect(),
            CellPayload::Json(payload),
            CellCost::new(0, 0)?,
        )?;

        cell.dependencies.push(CellDependency::new(
            source,
            CellDependencyKind::DerivedFrom,
            "revision link source StateCell",
        ));
        cell.dependencies.push(CellDependency::new(
            target,
            CellDependencyKind::DerivedFrom,
            "revision link target StateCell",
        ));

        let link_cell_id = cell.id;
        self.kernel.append_cell_at(cell, committed_at)?;
        Ok(Some(link_cell_id))
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

    /// Runs deterministic conflict-resolution stewardship and records proposal audit StateCells.
    #[cfg(feature = "steward")]
    pub fn audit_conflict_resolutions_with_steward<I>(
        &mut self,
        cell_ids: I,
        steward: &ConflictResolutionSteward,
        policy: &ProposalPolicy,
        decided_at: DateTime<Utc>,
    ) -> Result<StewardConflictAudit, ContinuityError>
    where
        I: IntoIterator<Item = StateCellId>,
    {
        let cells = self.lookup_cells_in_order(cell_ids)?;
        let scan = recommend_conflict_resolutions(&cells);
        let proposals = steward.propose_from_scan(&scan, decided_at)?;
        let mut records = Vec::with_capacity(proposals.len());
        let store = BorrowedKernelProposalStore::new(&mut self.kernel);
        let mut ledger = StoredProposalLedger::new(store);

        for proposal in proposals.iter().cloned() {
            let decision = policy.evaluate(&proposal, decided_at);
            let record = ProposalAuditRecord::new(proposal.clone(), decision.clone())?;
            ledger.record(proposal, decision)?;
            records.push(record);
        }

        Ok(StewardConflictAudit {
            scan,
            proposals,
            records,
        })
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

    fn validate_commit_export_batch(
        &self,
        batch: &CommitExportBatch,
    ) -> Result<(), ContinuityError> {
        let mut commit_ids = HashSet::new();
        let mut cell_ids = HashSet::new();
        for slice in &batch.slices {
            let commit_id = slice.manifest.commit_id;
            if !commit_ids.insert(commit_id) {
                return Err(ContinuityError::InvalidCommitExport { commit_id });
            }
            if self.commit_manifest(commit_id)?.is_some() {
                return Err(ContinuityError::Kernel(KernelError::DuplicateCommit));
            }

            let exported_ids = slice.cells.iter().map(|cell| cell.id).collect::<Vec<_>>();
            if exported_ids != slice.manifest.cell_ids {
                return Err(ContinuityError::InvalidCommitExport { commit_id });
            }

            for cell in &slice.cells {
                if cell.commit_id != commit_id
                    || cell.system_time.from() != slice.manifest.committed_at
                {
                    return Err(ContinuityError::InvalidCommitExport { commit_id });
                }
                if !cell_ids.insert(cell.id) {
                    return Err(ContinuityError::InvalidCommitExport { commit_id });
                }
                if !self
                    .kernel
                    .lookup_cells(CellLookup {
                        cell_id: Some(cell.id),
                        ..CellLookup::default()
                    })?
                    .is_empty()
                {
                    return Err(ContinuityError::Kernel(KernelError::DuplicateCell));
                }
            }
        }
        Ok(())
    }
}

fn decode_query_file(bytes: &[u8]) -> Result<ContinuityQuery, ContinuityError> {
    if let Some(text) = query_text_input(bytes) {
        return parse_query_text(text).map_err(Into::into);
    }
    if is_query_envelope_shape(bytes)? {
        return decode_query_json(bytes).map_err(Into::into);
    }
    serde_json::from_slice::<ContinuityQuery>(bytes).map_err(|_error| ContinuityError::QueryJson)
}

fn query_text_input(bytes: &[u8]) -> Option<&str> {
    let text = std::str::from_utf8(bytes).ok()?;
    let trimmed = text.trim_start();
    if trimmed.len() >= "checkout".len()
        && trimmed[.."checkout".len()].eq_ignore_ascii_case("checkout")
    {
        Some(text)
    } else {
        None
    }
}

fn is_query_envelope_shape(bytes: &[u8]) -> Result<bool, ContinuityError> {
    let value = serde_json::from_slice::<serde_json::Value>(bytes)
        .map_err(|_error| ContinuityError::QueryJson)?;
    Ok(value.get("format").is_some()
        && value.get("version").is_some()
        && value.get("query").is_some())
}

impl ContinuityDb<FileKernel> {
    /// Opens a file-backed ContinuityDB instance.
    pub fn open_file<P: AsRef<Path>>(path: P) -> Result<Self, ContinuityError> {
        FileKernel::open(path).map(Self::new).map_err(Into::into)
    }

    /// Opens a file-backed ContinuityDB instance only when the kernel satisfies the requested guarantees.
    pub fn open_file_with_requirements<P: AsRef<Path>>(
        path: P,
        requirements: KernelRequirements,
    ) -> Result<Self, ContinuityError> {
        let db = Self::open_file(path)?;
        db.ensure_kernel_requirements(requirements)?;
        Ok(db)
    }

    /// Opens a file-backed ContinuityDB instance only when the store is already canonical.
    pub fn open_canonical_file<P: AsRef<Path>>(path: P) -> Result<Self, ContinuityError> {
        let db = Self::open_file(path)?;
        db.ensure_file_store_canonical()?;
        Ok(db)
    }

    /// Rewrites a file-backed store into the current canonical durable record format.
    pub fn compact_file_store(&mut self) -> Result<(), ContinuityError> {
        self.kernel.compact().map_err(Into::into)
    }

    /// Rewrites a file-backed store only when health recommends compaction.
    pub fn compact_file_store_if_needed(
        &mut self,
    ) -> Result<FileCompactionSummary, ContinuityError> {
        let before = self.file_store_health();
        if !before.compaction_recommended {
            return Ok(FileCompactionSummary {
                compacted: false,
                before,
                after: before,
            });
        }

        self.compact_file_store()?;
        let after = self.file_store_health();
        Ok(FileCompactionSummary {
            compacted: true,
            before,
            after,
        })
    }

    /// Returns observable status for the backing file store.
    pub fn file_store_status(&self) -> Result<FileKernelStatus, ContinuityError> {
        self.kernel.status().map_err(Into::into)
    }

    /// Returns the file-format health report for the backing file store.
    pub fn file_store_health(&self) -> FileKernelHealth {
        self.kernel.health()
    }

    /// Ensures the file-backed store does not require compaction.
    pub fn ensure_file_store_canonical(&self) -> Result<(), ContinuityError> {
        let health = self.file_store_health();
        if health.compaction_recommended {
            Err(ContinuityError::FileStoreCompactionRecommended { health })
        } else {
            Ok(())
        }
    }

    /// Writes a versioned JSON commit export envelope to a file.
    pub fn export_commits_json_file<P: AsRef<Path>>(
        &self,
        lookup: CommitManifestLookup,
        output_path: P,
    ) -> Result<CommitExportFileSummary, ContinuityError> {
        let batch = self.export_commits(lookup)?;
        let summary = CommitExportFileSummary {
            exported_commits: batch.slices.len(),
            next_after: batch.next_after,
        };
        let encoded = Self::encode_commit_export_json(batch)?;
        fs::write(output_path, encoded).map_err(|_error| ContinuityError::CommitExportFileIo)?;
        Ok(summary)
    }

    /// Imports a versioned JSON commit export envelope from a file.
    pub fn import_commits_json_file<P: AsRef<Path>>(
        &mut self,
        input_path: P,
    ) -> Result<usize, ContinuityError> {
        let summary = self.import_commits_json_file_with_summary(input_path)?;
        Ok(summary.imported_commits)
    }

    /// Imports a versioned JSON commit export envelope from a file and returns cursor metadata.
    pub fn import_commits_json_file_with_summary<P: AsRef<Path>>(
        &mut self,
        input_path: P,
    ) -> Result<CommitImportSummary, ContinuityError> {
        let encoded = fs::read(input_path).map_err(|_error| ContinuityError::CommitExportFileIo)?;
        let batch = Self::decode_commit_export_json(&encoded)?;
        self.import_commit_batch_with_summary(batch)
    }

    /// Validates a versioned JSON commit export envelope from a file without mutating the store.
    pub fn validate_commits_json_file<P: AsRef<Path>>(
        &self,
        input_path: P,
    ) -> Result<CommitImportValidation, ContinuityError> {
        let encoded = fs::read(input_path).map_err(|_error| ContinuityError::CommitExportFileIo)?;
        let batch = Self::decode_commit_export_json(&encoded)?;
        self.validate_commit_import(&batch)
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use continuitydb_checkout::CheckoutRequest;
    #[cfg(feature = "steward")]
    use continuitydb_core::{ActivationState, CellDependencyKind};
    use continuitydb_core::{
        Answerability, CellCost, CellPayload, Citation, CommitId, Confidence, Evidence, Scope,
        SemanticAnchor, SourceId, StateCell, StateCellId, TrustSignal, UtilityFeedback,
        ValidTimeRange,
    };
    use continuitydb_kernel::{
        CellLookup, CommitManifestLookup, FileKernel, KernelDurability, KernelError,
        KernelRequirements, StorageKernel,
    };
    use continuitydb_memory::MemoryKernel;
    use continuitydb_query::{
        encode_query_json, CheckoutQuery, ContinuityQuery, QueryEnvelope, QueryEnvelopeError,
        QueryError, QueryOptimization, QueryRequirements, QueryReturnShape, QueryTask,
        QueryTextError, QUERY_ENVELOPE_FORMAT, QUERY_ENVELOPE_FORMAT_VERSION,
    };
    #[cfg(feature = "steward")]
    use continuitydb_revision::RevisionLinkKind;
    #[cfg(feature = "steward")]
    use continuitydb_steward::{
        ConflictResolutionSteward, FrontierSteward, FrontierSubscription, FrontierSubscriptionId,
        FrontierSubscriptionRunner, FrontierSubscriptionStore, FrontierWatchEvent,
        FrontierWatchSignal, MemoryFrontierSubscriptionStore, ProposalId, ProposalOutcome,
        ProposalPolicy, StewardAction, StewardIdentity, StewardProposal,
    };
    use std::{fs, path::Path};

    use super::{
        CommitExportBatch, CommitExportFileSummary, CommitImportSummary, CommitSlice, ContinuityDb,
        ContinuityError,
    };

    fn sample_cell(
        anchor: &str,
        confidence: f32,
        tokens: i64,
    ) -> Result<StateCell, Box<dyn std::error::Error>> {
        sample_cell_with_payload_day_and_confidence(anchor, anchor, 20, confidence, tokens)
    }

    fn temp_file_kernel_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("{name}-{:?}.jsonl", StateCellId::new()))
    }

    fn write_legacy_file_store(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let committed_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let commit_id = CommitId::new();
        let mut cell = sample_cell("project:continuitydb:api-canonical-legacy", 0.91, 12)?;
        cell.system_time = continuitydb_core::SystemTimeRange::open_from(committed_at);
        cell.commit_id = commit_id;
        fs::write(path, format!("{}\n", serde_json::to_string(&cell)?))?;
        Ok(())
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

    #[cfg(feature = "steward")]
    fn test_steward_time() -> Result<chrono::DateTime<Utc>, Box<dyn std::error::Error>> {
        Utc.with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp").into())
    }

    #[cfg(feature = "steward")]
    fn test_steward_identity() -> Result<StewardIdentity, Box<dyn std::error::Error>> {
        Ok(StewardIdentity::new(
            "native-api-steward",
            "0.1.0",
            "strict",
        )?)
    }

    #[cfg(feature = "steward")]
    fn test_conflict_steward() -> Result<ConflictResolutionSteward, Box<dyn std::error::Error>> {
        Ok(ConflictResolutionSteward::new(StewardIdentity::new(
            "native-api-conflict-steward",
            "0.1.0",
            "deterministic-policy",
        )?))
    }

    #[cfg(feature = "steward")]
    fn test_frontier_runner(
        cell_id: StateCellId,
        signal: FrontierWatchSignal,
    ) -> Result<
        FrontierSubscriptionRunner<MemoryFrontierSubscriptionStore>,
        Box<dyn std::error::Error>,
    > {
        let subscription = FrontierSubscription::new(
            FrontierSubscriptionId::new(),
            cell_id,
            vec![signal],
            "test://frontier-subscription",
            test_steward_time()?,
        )?;
        let mut store = MemoryFrontierSubscriptionStore::default();
        store.append_subscription(subscription)?;
        Ok(FrontierSubscriptionRunner::new(
            FrontierSteward::new(test_steward_identity()?),
            store,
        ))
    }

    #[cfg(feature = "steward")]
    fn sample_steward_proposal(
        rationale: &str,
    ) -> Result<StewardProposal, Box<dyn std::error::Error>> {
        Ok(StewardProposal::new(
            ProposalId::new(),
            test_steward_identity()?,
            StewardAction::LinkRevision {
                source: StateCellId::new(),
                kind: RevisionLinkKind::Supersedes,
                target: StateCellId::new(),
            },
            rationale,
            vec!["test://steward".to_string()],
            test_steward_time()?,
        )?)
    }

    #[test]
    fn api_exposes_kernel_capabilities() {
        let db = ContinuityDb::new(MemoryKernel::default());

        let capabilities = db.kernel_capabilities();

        assert_eq!(KernelDurability::Ephemeral, capabilities.durability);
        assert!(capabilities.append_only);
        assert!(!capabilities.durable_flush);
    }

    #[test]
    fn api_accepts_satisfied_kernel_requirements() -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_file_kernel_path("api-kernel-requirements");
        let db = ContinuityDb::new(FileKernel::open(&path)?);

        assert!(db.kernel_satisfies(KernelRequirements::durable_append_log()));
        db.ensure_kernel_requirements(KernelRequirements::durable_append_log())?;

        let _ = std::fs::remove_file(path);
        Ok(())
    }

    #[test]
    fn api_rejects_unsatisfied_kernel_requirements() {
        let db = ContinuityDb::new(MemoryKernel::default());
        let required = KernelRequirements::durable_append_log();
        let actual = db.kernel_capabilities();

        let result = db.ensure_kernel_requirements(required);

        assert!(!db.kernel_satisfies(required));
        assert!(matches!(
            result,
            Err(ContinuityError::KernelRequirementsNotMet {
                required: error_required,
                actual: error_actual,
            }) if error_required == required && error_actual == actual
        ));
    }

    #[test]
    fn api_opens_file_backed_database() -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_file_kernel_path("api-open-file");

        let db = ContinuityDb::open_file(&path)?;

        assert_eq!(db.kernel().path(), path.as_path());
        assert!(db.kernel_satisfies(KernelRequirements::durable_append_log()));

        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn api_reports_file_store_status() -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_file_kernel_path("api-file-store-status");
        let mut db = ContinuityDb::open_file(&path)?;
        db.ingest_cells_at_with_commit_id(
            vec![sample_cell("project:continuitydb:file-status", 0.91, 12)?],
            Utc.with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
                .single()
                .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?,
            CommitId::new(),
        )?;

        let status = db.file_store_status()?;

        assert_eq!(status.cell_count, 1);
        assert_eq!(status.commit_count, 1);
        assert!(status.file_size_bytes > 0);

        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn api_reports_file_store_health() -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_file_kernel_path("api-file-store-health");
        let db = ContinuityDb::open_file(&path)?;

        let health = db.file_store_health();

        assert!(health.has_header);
        assert_eq!(health.legacy_raw_cells, 0);
        assert_eq!(health.checksum_free_records, 0);
        assert_eq!(health.canonical_records, 0);
        assert!(!health.compaction_recommended);

        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn api_accepts_canonical_file_store() -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_file_kernel_path("api-file-store-canonical");
        let db = ContinuityDb::open_file(&path)?;

        db.ensure_file_store_canonical()?;

        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn api_rejects_legacy_file_store_when_canonical_required(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_file_kernel_path("api-file-store-canonical-legacy");
        write_legacy_file_store(&path)?;
        let db = ContinuityDb::open_file(&path)?;

        let result = db.ensure_file_store_canonical();

        assert!(matches!(
            result,
            Err(ContinuityError::FileStoreCompactionRecommended { health })
                if health.legacy_raw_cells == 1 && health.compaction_recommended
        ));

        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn api_open_canonical_file_rejects_legacy_file_store() -> Result<(), Box<dyn std::error::Error>>
    {
        let path = temp_file_kernel_path("api-open-canonical-file-legacy");
        write_legacy_file_store(&path)?;

        let result = ContinuityDb::open_canonical_file(&path);

        assert!(matches!(
            result,
            Err(ContinuityError::FileStoreCompactionRecommended { health })
                if health.legacy_raw_cells == 1 && health.compaction_recommended
        ));

        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn api_open_file_accepts_satisfied_kernel_requirements(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_file_kernel_path("api-open-file-require-durable");

        let db = ContinuityDb::open_file_with_requirements(
            &path,
            KernelRequirements::durable_append_log(),
        )?;

        assert_eq!(db.kernel().path(), path.as_path());

        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn api_open_file_rejects_unsatisfied_kernel_requirements() {
        let path = temp_file_kernel_path("api-open-file-require-indexed");
        let required = KernelRequirements::indexed_embedded();

        let result = ContinuityDb::open_file_with_requirements(&path, required);

        assert!(matches!(
            result,
            Err(ContinuityError::KernelRequirementsNotMet {
                required: error_required,
                actual,
            }) if error_required == required
                && actual.durability == continuitydb_kernel::KernelDurability::AppendLog
                && !actual.persistent_indexes
        ));

        let _ = fs::remove_file(path);
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
            semantic_anchor: None,
            scope: Some(Scope::Project("continuitydb".to_string())),
            valid_at: Some(committed_at),
            system_at: Some(committed_at),
            commit_id: None,
            activation: None,
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
    fn api_checkout_query_materializes_slice() -> Result<(), Box<dyn std::error::Error>> {
        let committed_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let cell = sample_cell("project:continuitydb:api-query", 0.91, 12)?;
        let cell_id = cell.id;
        db.ingest_cell_at(cell, committed_at)?;

        let query = CheckoutQuery::new(QueryTask::new("api-query", "what should the agent know?"))
            .with_requirements(QueryRequirements {
                scope: Some(Scope::Project("continuitydb".to_string())),
                valid_at: Some(committed_at),
                system_at: Some(committed_at),
                minimum_confidence: Confidence::new(0.8)?,
                token_budget: 100,
                ..QueryRequirements::default()
            });
        let slice = db.checkout_query(query)?;

        assert_eq!(slice.cells.len(), 1);
        assert_eq!(slice.cells[0].id, cell_id);
        Ok(())
    }

    #[test]
    fn api_checkout_query_applies_token_budget_to_alternatives(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let committed_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let first = sample_cell("project:continuitydb:api-query-budget-first", 0.91, 80)?;
        let second = sample_cell("project:continuitydb:api-query-budget-second", 0.9, 80)?;
        db.ingest_cells_at(vec![first, second], committed_at)?;

        let query = CheckoutQuery::new(QueryTask::new(
            "api-query-budget",
            "what should the agent know?",
        ))
        .with_requirements(QueryRequirements {
            scope: Some(Scope::Project("continuitydb".to_string())),
            minimum_confidence: Confidence::new(0.8)?,
            token_budget: 100,
            ..QueryRequirements::default()
        });
        let slice = db.checkout_query(query)?;

        assert_eq!(slice.cells.len(), 1);
        assert_eq!(slice.alternatives.len(), 1);
        Ok(())
    }

    #[test]
    fn api_checkout_continuity_query_delegates_top_level_query(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let committed_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let cell = sample_cell("project:continuitydb:api-continuity-query", 0.91, 12)?;
        let cell_id = cell.id;
        db.ingest_cell_at(cell, committed_at)?;

        let query = ContinuityQuery::Checkout(CheckoutQuery::new(QueryTask::new(
            "api-continuity-query",
            "what should the agent know?",
        )));
        let slice = db.checkout_continuity_query(query)?;

        assert_eq!(slice.cells.len(), 1);
        assert_eq!(slice.cells[0].id, cell_id);
        Ok(())
    }

    #[test]
    fn api_checkout_query_json_materializes_slice() -> Result<(), Box<dyn std::error::Error>> {
        let mut db = ContinuityDb::new(MemoryKernel::default());
        db.ingest_cell(sample_cell("project:continuitydb:query-json", 0.91, 12)?)?;
        let query = ContinuityQuery::Checkout(CheckoutQuery::new(QueryTask::new(
            "stored-facts",
            "what should the agent know?",
        )));
        let encoded = encode_query_json(query)?;

        let slice = db.checkout_query_json(&encoded)?;

        assert_eq!(slice.cells.len(), 1);
        assert_eq!(
            slice.cells[0].payload,
            CellPayload::Text("project:continuitydb:query-json".to_string())
        );
        Ok(())
    }

    #[test]
    fn api_checkout_query_json_preserves_query_compilation_errors(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let db = ContinuityDb::new(MemoryKernel::default());
        let query = ContinuityQuery::Checkout(
            CheckoutQuery::new(QueryTask::new(
                "stored-facts",
                "what should the agent know?",
            ))
            .with_return_shape(QueryReturnShape::CellsOnly),
        );
        let encoded = encode_query_json(query)?;

        let result = db.checkout_query_json(&encoded);

        assert_eq!(
            result.err(),
            Some(ContinuityError::Query(QueryError::UnsupportedReturnShape(
                QueryReturnShape::CellsOnly
            )))
        );
        Ok(())
    }

    #[test]
    fn api_checkout_query_json_reports_invalid_json() {
        let db = ContinuityDb::new(MemoryKernel::default());

        let result = db.checkout_query_json(b"{not valid json}\n");

        assert_eq!(
            result.err(),
            Some(ContinuityError::QueryEnvelope(
                QueryEnvelopeError::InvalidJson
            ))
        );
    }

    #[test]
    fn api_checkout_query_json_reports_invalid_envelope() -> Result<(), Box<dyn std::error::Error>>
    {
        let db = ContinuityDb::new(MemoryKernel::default());
        let envelope = QueryEnvelope {
            format: QUERY_ENVELOPE_FORMAT.to_string(),
            version: QUERY_ENVELOPE_FORMAT_VERSION + 1,
            query: ContinuityQuery::Checkout(CheckoutQuery::new(QueryTask::new(
                "stored-facts",
                "what should the agent know?",
            ))),
        };
        let encoded = serde_json::to_vec(&envelope)?;

        let result = db.checkout_query_json(&encoded);

        assert_eq!(
            result.err(),
            Some(ContinuityError::QueryEnvelope(
                QueryEnvelopeError::InvalidEnvelope
            ))
        );
        Ok(())
    }

    #[test]
    fn api_checkout_query_text_materializes_slice() -> Result<(), Box<dyn std::error::Error>> {
        let mut db = ContinuityDb::new(MemoryKernel::default());
        db.ingest_cell(sample_cell("project:continuitydb:query-text", 0.91, 12)?)?;

        let slice = db.checkout_query_text(
            r#"CHECKOUT "stored-facts" ANSWER "what should the agent know?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
  AND token_budget <= 1200"#,
        )?;

        assert_eq!(slice.cells.len(), 1);
        assert_eq!(
            slice.cells[0].payload,
            CellPayload::Text("project:continuitydb:query-text".to_string())
        );
        Ok(())
    }

    #[test]
    fn api_checkout_query_text_reports_invalid_text_query() {
        let db = ContinuityDb::new(MemoryKernel::default());

        let result = db.checkout_query_text(r#"CHECKOUT "stored-facts" WHERE scope = global"#);

        assert_eq!(
            result.err(),
            Some(ContinuityError::QueryText(QueryTextError::InvalidSyntax))
        );
    }

    #[test]
    fn api_checkout_query_file_executes_query_envelope() -> Result<(), Box<dyn std::error::Error>> {
        let mut db = ContinuityDb::new(MemoryKernel::default());
        db.ingest_cell(sample_cell(
            "project:continuitydb:query-file-envelope",
            0.91,
            12,
        )?)?;
        let query = ContinuityQuery::Checkout(CheckoutQuery::new(QueryTask::new(
            "stored-facts",
            "what should the agent know?",
        )));
        let path = temp_file_kernel_path("query-file-envelope");
        fs::write(&path, encode_query_json(query)?)?;

        let slice = db.checkout_query_file(&path)?;

        assert_eq!(slice.cells.len(), 1);
        assert_eq!(
            slice.cells[0].payload,
            CellPayload::Text("project:continuitydb:query-file-envelope".to_string())
        );
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn api_checkout_query_file_executes_raw_query_json() -> Result<(), Box<dyn std::error::Error>> {
        let mut db = ContinuityDb::new(MemoryKernel::default());
        db.ingest_cell(sample_cell(
            "project:continuitydb:query-file-raw",
            0.91,
            12,
        )?)?;
        let query = ContinuityQuery::Checkout(CheckoutQuery::new(QueryTask::new(
            "stored-facts",
            "what should the agent know?",
        )));
        let path = temp_file_kernel_path("query-file-raw");
        fs::write(&path, serde_json::to_vec(&query)?)?;

        let slice = db.checkout_query_file(&path)?;

        assert_eq!(slice.cells.len(), 1);
        assert_eq!(
            slice.cells[0].payload,
            CellPayload::Text("project:continuitydb:query-file-raw".to_string())
        );
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn api_checkout_query_file_executes_text_query() -> Result<(), Box<dyn std::error::Error>> {
        let mut db = ContinuityDb::new(MemoryKernel::default());
        db.ingest_cell(sample_cell(
            "project:continuitydb:query-file-text",
            0.91,
            12,
        )?)?;
        let path = temp_file_kernel_path("query-file-text");
        fs::write(
            &path,
            r#"CHECKOUT "stored-facts" ANSWER "what should the agent know?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
  AND token_budget <= 1200"#,
        )?;

        let slice = db.checkout_query_file(&path)?;

        assert_eq!(slice.cells.len(), 1);
        assert_eq!(
            slice.cells[0].payload,
            CellPayload::Text("project:continuitydb:query-file-text".to_string())
        );
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn api_checkout_query_file_reports_invalid_text_query() -> Result<(), Box<dyn std::error::Error>>
    {
        let db = ContinuityDb::new(MemoryKernel::default());
        let path = temp_file_kernel_path("invalid-query-file-text");
        fs::write(&path, r#"CHECKOUT "stored-facts" WHERE scope = global"#)?;

        let result = db.checkout_query_file(&path);

        assert_eq!(
            result.err(),
            Some(ContinuityError::QueryText(QueryTextError::InvalidSyntax))
        );
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn api_checkout_query_file_reports_missing_file() {
        let db = ContinuityDb::new(MemoryKernel::default());
        let path = temp_file_kernel_path("missing-query-file");

        let result = db.checkout_query_file(&path);

        assert_eq!(result.err(), Some(ContinuityError::QueryFileIo));
    }

    #[test]
    fn api_checkout_query_file_reports_invalid_raw_json() -> Result<(), Box<dyn std::error::Error>>
    {
        let db = ContinuityDb::new(MemoryKernel::default());
        let path = temp_file_kernel_path("invalid-query-file-json");
        fs::write(&path, b"{not valid json}\n")?;

        let result = db.checkout_query_file(&path);

        assert_eq!(result.err(), Some(ContinuityError::QueryJson));
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn api_checkout_query_file_reports_invalid_envelope() -> Result<(), Box<dyn std::error::Error>>
    {
        let db = ContinuityDb::new(MemoryKernel::default());
        let envelope = QueryEnvelope {
            format: QUERY_ENVELOPE_FORMAT.to_string(),
            version: QUERY_ENVELOPE_FORMAT_VERSION + 1,
            query: ContinuityQuery::Checkout(CheckoutQuery::new(QueryTask::new(
                "stored-facts",
                "what should the agent know?",
            ))),
        };
        let path = temp_file_kernel_path("invalid-query-file-envelope");
        fs::write(&path, serde_json::to_vec(&envelope)?)?;

        let result = db.checkout_query_file(&path);

        assert_eq!(
            result.err(),
            Some(ContinuityError::QueryEnvelope(
                QueryEnvelopeError::InvalidEnvelope
            ))
        );
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn api_checkout_query_returns_unsupported_shape_error() {
        let db = ContinuityDb::new(MemoryKernel::default());
        let query = CheckoutQuery::new(QueryTask::new(
            "api-query-shape",
            "what should the agent know?",
        ))
        .with_return_shape(QueryReturnShape::CellsOnly);

        let result = db.checkout_query(query);

        assert!(matches!(
            result,
            Err(ContinuityError::Query(QueryError::UnsupportedReturnShape(
                QueryReturnShape::CellsOnly
            )))
        ));
    }

    #[test]
    fn api_checkout_query_returns_unsupported_optimization_error() {
        let db = ContinuityDb::new(MemoryKernel::default());
        let query = CheckoutQuery::new(QueryTask::new(
            "api-query-optimization",
            "what should the agent know?",
        ))
        .with_optimization(QueryOptimization::TokenCostOnly);

        let result = db.checkout_query(query);

        assert!(matches!(
            result,
            Err(ContinuityError::Query(QueryError::UnsupportedOptimization(
                QueryOptimization::TokenCostOnly
            )))
        ));
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
    fn api_returns_commit_cells_in_manifest_order() -> Result<(), Box<dyn std::error::Error>> {
        let committed_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let commit_id = CommitId::new();
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let first = sample_cell("project:continuitydb:commit-cells-first", 0.91, 12)?;
        let second = sample_cell("project:continuitydb:commit-cells-second", 0.83, 15)?;
        let expected_ids = vec![first.id, second.id];

        db.ingest_cells_at_with_commit_id(vec![first, second], committed_at, commit_id)?;

        let cells = db.commit_cells(commit_id)?;
        assert_eq!(
            cells.iter().map(|cell| cell.id).collect::<Vec<_>>(),
            expected_ids
        );
        assert!(cells.iter().all(|cell| cell.commit_id == commit_id));
        Ok(())
    }

    #[test]
    fn api_commit_cells_reports_unknown_commit() {
        let db = ContinuityDb::new(MemoryKernel::default());

        let result = db.commit_cells(CommitId::new());

        assert!(matches!(
            result,
            Err(ContinuityError::Kernel(KernelError::CommitNotFound))
        ));
    }

    #[test]
    fn api_returns_commit_slices_after_cursor_with_limit() -> Result<(), Box<dyn std::error::Error>>
    {
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
        let first = sample_cell("project:continuitydb:slice-first", 0.91, 12)?;
        let second_a = sample_cell("project:continuitydb:slice-second-a", 0.83, 15)?;
        let second_b = sample_cell("project:continuitydb:slice-second-b", 0.82, 16)?;
        let third = sample_cell("project:continuitydb:slice-third", 0.77, 18)?;
        let expected_second_ids = vec![second_a.id, second_b.id];

        db.ingest_cells_at_with_commit_id(vec![first], first_time, first_commit)?;
        db.ingest_cells_at_with_commit_id(vec![second_a, second_b], second_time, second_commit)?;
        db.ingest_cells_at_with_commit_id(vec![third], third_time, third_commit)?;

        let slices = db.commit_slices(CommitManifestLookup {
            after: Some(first_commit),
            limit: Some(1),
        })?;

        assert_eq!(slices.len(), 1);
        assert_eq!(slices[0].manifest.commit_id, second_commit);
        assert_eq!(slices[0].manifest.committed_at, second_time);
        assert_eq!(slices[0].manifest.cell_ids, expected_second_ids);
        assert_eq!(
            slices[0]
                .cells
                .iter()
                .map(|cell| cell.id)
                .collect::<Vec<_>>(),
            expected_second_ids
        );
        assert!(slices[0]
            .cells
            .iter()
            .all(|cell| cell.commit_id == second_commit));
        Ok(())
    }

    #[test]
    fn api_commit_slices_reports_unknown_cursor() {
        let db = ContinuityDb::new(MemoryKernel::default());

        let result = db.commit_slices(CommitManifestLookup {
            after: Some(CommitId::new()),
            limit: Some(10),
        });

        assert!(matches!(
            result,
            Err(ContinuityError::Kernel(KernelError::CommitNotFound))
        ));
    }

    #[test]
    fn api_commit_slices_returns_empty_for_empty_database() -> Result<(), Box<dyn std::error::Error>>
    {
        let db = ContinuityDb::new(MemoryKernel::default());

        let slices = db.commit_slices(CommitManifestLookup::default())?;

        assert!(slices.is_empty());
        Ok(())
    }

    #[test]
    fn api_exports_commit_batch_with_next_cursor() -> Result<(), Box<dyn std::error::Error>> {
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
        let first = sample_cell("project:continuitydb:export-first", 0.91, 12)?;
        let second_a = sample_cell("project:continuitydb:export-second-a", 0.83, 15)?;
        let second_b = sample_cell("project:continuitydb:export-second-b", 0.82, 16)?;
        let expected_second_ids = vec![second_a.id, second_b.id];

        db.ingest_cells_at_with_commit_id(vec![first], first_time, first_commit)?;
        db.ingest_cells_at_with_commit_id(vec![second_a, second_b], second_time, second_commit)?;

        let batch = db.export_commits(CommitManifestLookup {
            after: Some(first_commit),
            limit: Some(1),
        })?;

        assert_eq!(batch.next_after, Some(second_commit));
        assert_eq!(batch.slices.len(), 1);
        assert_eq!(batch.slices[0].manifest.commit_id, second_commit);
        assert_eq!(
            batch.slices[0]
                .cells
                .iter()
                .map(|cell| cell.id)
                .collect::<Vec<_>>(),
            expected_second_ids
        );
        Ok(())
    }

    #[test]
    fn api_exports_empty_commit_batch() -> Result<(), Box<dyn std::error::Error>> {
        let db = ContinuityDb::new(MemoryKernel::default());

        let batch = db.export_commits(CommitManifestLookup::default())?;

        assert_eq!(
            batch,
            CommitExportBatch {
                slices: Vec::new(),
                next_after: None,
            }
        );
        Ok(())
    }

    #[test]
    fn api_export_commits_reports_unknown_cursor() {
        let db = ContinuityDb::new(MemoryKernel::default());

        let result = db.export_commits(CommitManifestLookup {
            after: Some(CommitId::new()),
            limit: Some(10),
        });

        assert!(matches!(
            result,
            Err(ContinuityError::Kernel(KernelError::CommitNotFound))
        ));
    }

    #[test]
    fn api_encodes_and_decodes_commit_export_json() -> Result<(), Box<dyn std::error::Error>> {
        let committed_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let commit_id = CommitId::new();
        let mut db = ContinuityDb::new(MemoryKernel::default());
        db.ingest_cells_at_with_commit_id(
            vec![sample_cell("project:continuitydb:json-export", 0.91, 12)?],
            committed_at,
            commit_id,
        )?;
        let batch = db.export_commits(CommitManifestLookup::default())?;

        let encoded = ContinuityDb::<MemoryKernel>::encode_commit_export_json(batch.clone())?;
        let envelope: serde_json::Value = serde_json::from_slice(&encoded)?;
        let decoded = ContinuityDb::<MemoryKernel>::decode_commit_export_json(&encoded)?;

        assert_eq!(envelope["format"], "continuitydb.commit_export");
        assert_eq!(envelope["version"], 1);
        assert_eq!(decoded, batch);
        Ok(())
    }

    #[test]
    fn api_rejects_unsupported_commit_export_json_version() {
        let encoded = br#"{"format":"continuitydb.commit_export","version":999,"batch":{"slices":[],"next_after":null}}"#;

        let result = ContinuityDb::<MemoryKernel>::decode_commit_export_json(encoded);

        assert!(matches!(
            result,
            Err(ContinuityError::InvalidCommitExportEnvelope)
        ));
    }

    #[test]
    fn api_imports_decoded_commit_export_json() -> Result<(), Box<dyn std::error::Error>> {
        let committed_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let commit_id = CommitId::new();
        let mut source = ContinuityDb::new(MemoryKernel::default());
        source.ingest_cells_at_with_commit_id(
            vec![sample_cell("project:continuitydb:json-import", 0.91, 12)?],
            committed_at,
            commit_id,
        )?;
        let batch = source.export_commits(CommitManifestLookup::default())?;
        let encoded = ContinuityDb::<MemoryKernel>::encode_commit_export_json(batch.clone())?;
        let decoded = ContinuityDb::<MemoryKernel>::decode_commit_export_json(&encoded)?;
        let mut target = ContinuityDb::new(MemoryKernel::default());

        let imported = target.import_commit_batch(decoded)?;

        assert_eq!(imported, 1);
        assert_eq!(
            target.export_commits(CommitManifestLookup::default())?,
            batch
        );
        Ok(())
    }

    #[test]
    fn api_validates_commit_import_without_mutation() -> Result<(), Box<dyn std::error::Error>> {
        let committed_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let commit_id = CommitId::new();
        let mut source = ContinuityDb::new(MemoryKernel::default());
        source.ingest_cells_at_with_commit_id(
            vec![sample_cell(
                "project:continuitydb:dry-run-source",
                0.91,
                12,
            )?],
            committed_at,
            commit_id,
        )?;
        let batch = source.export_commits(CommitManifestLookup::default())?;
        let target = ContinuityDb::new(MemoryKernel::default());

        let validation = target.validate_commit_import(&batch)?;

        assert_eq!(validation.valid_commits, 1);
        assert_eq!(
            target.commit_slices(CommitManifestLookup::default())?.len(),
            0
        );
        Ok(())
    }

    #[test]
    fn api_validate_commit_import_reports_duplicate_commit(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let committed_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let commit_id = CommitId::new();
        let mut source = ContinuityDb::new(MemoryKernel::default());
        source.ingest_cells_at_with_commit_id(
            vec![sample_cell(
                "project:continuitydb:dry-run-duplicate",
                0.91,
                12,
            )?],
            committed_at,
            commit_id,
        )?;
        let batch = source.export_commits(CommitManifestLookup::default())?;
        let mut target = ContinuityDb::new(MemoryKernel::default());
        target.ingest_cells_at_with_commit_id(
            vec![sample_cell(
                "project:continuitydb:dry-run-existing",
                0.83,
                15,
            )?],
            committed_at,
            commit_id,
        )?;

        let result = target.validate_commit_import(&batch);

        assert!(matches!(
            result,
            Err(ContinuityError::Kernel(KernelError::DuplicateCommit))
        ));
        assert_eq!(
            target.commit_slices(CommitManifestLookup::default())?.len(),
            1
        );
        Ok(())
    }

    #[test]
    fn api_exports_commit_backup_json_file() -> Result<(), Box<dyn std::error::Error>> {
        let source_path = temp_file_kernel_path("continuitydb-api-export-backup-source");
        let backup_path = temp_file_kernel_path("continuitydb-api-export-backup-file");
        let committed_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let commit_id = CommitId::new();
        let mut db = ContinuityDb::new(FileKernel::open(&source_path)?);
        db.ingest_cells_at_with_commit_id(
            vec![sample_cell("project:continuitydb:file-export", 0.91, 12)?],
            committed_at,
            commit_id,
        )?;
        let batch = db.export_commits(CommitManifestLookup::default())?;

        let summary = db.export_commits_json_file(CommitManifestLookup::default(), &backup_path)?;
        let encoded = std::fs::read(&backup_path)?;
        let envelope: serde_json::Value = serde_json::from_slice(&encoded)?;
        let decoded = ContinuityDb::<FileKernel>::decode_commit_export_json(&encoded)?;

        assert_eq!(
            summary,
            CommitExportFileSummary {
                exported_commits: 1,
                next_after: batch.next_after,
            }
        );
        assert_eq!(envelope["format"], "continuitydb.commit_export");
        assert_eq!(envelope["version"], 1);
        assert_eq!(decoded, batch);

        std::fs::remove_file(source_path)?;
        std::fs::remove_file(backup_path)?;
        Ok(())
    }

    #[test]
    fn api_validates_commit_backup_json_file_without_mutation(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let source_path = temp_file_kernel_path("api-dry-run-source");
        let target_path = temp_file_kernel_path("api-dry-run-target");
        let backup_path = temp_file_kernel_path("api-dry-run-backup");
        let committed_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let commit_id = CommitId::new();
        let mut source = ContinuityDb::open_file(&source_path)?;
        source.ingest_cells_at_with_commit_id(
            vec![sample_cell("project:continuitydb:dry-run-file", 0.91, 12)?],
            committed_at,
            commit_id,
        )?;
        source.export_commits_json_file(CommitManifestLookup::default(), &backup_path)?;
        let target = ContinuityDb::open_file(&target_path)?;

        let validation = target.validate_commits_json_file(&backup_path)?;

        assert_eq!(validation.valid_commits, 1);
        assert_eq!(
            target.commit_slices(CommitManifestLookup::default())?.len(),
            0
        );

        fs::remove_file(source_path)?;
        fs::remove_file(target_path)?;
        fs::remove_file(backup_path)?;
        Ok(())
    }

    #[test]
    fn api_imports_commit_backup_json_file() -> Result<(), Box<dyn std::error::Error>> {
        let source_path = temp_file_kernel_path("continuitydb-api-import-backup-source");
        let target_path = temp_file_kernel_path("continuitydb-api-import-backup-target");
        let backup_path = temp_file_kernel_path("continuitydb-api-import-backup-file");
        let committed_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let commit_id = CommitId::new();
        let mut source = ContinuityDb::new(FileKernel::open(&source_path)?);
        source.ingest_cells_at_with_commit_id(
            vec![sample_cell("project:continuitydb:file-import", 0.91, 12)?],
            committed_at,
            commit_id,
        )?;
        source.export_commits_json_file(CommitManifestLookup::default(), &backup_path)?;
        let source_batch = source.export_commits(CommitManifestLookup::default())?;
        let mut target = ContinuityDb::new(FileKernel::open(&target_path)?);

        let imported = target.import_commits_json_file(&backup_path)?;

        assert_eq!(imported, 1);
        assert_eq!(
            target.export_commits(CommitManifestLookup::default())?,
            source_batch
        );

        std::fs::remove_file(source_path)?;
        std::fs::remove_file(target_path)?;
        std::fs::remove_file(backup_path)?;
        Ok(())
    }

    #[test]
    fn api_import_commit_backup_json_file_summary_reports_cursor(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let source_path = temp_file_kernel_path("continuitydb-api-import-summary-source");
        let target_path = temp_file_kernel_path("continuitydb-api-import-summary-target");
        let backup_path = temp_file_kernel_path("continuitydb-api-import-summary-backup");
        let committed_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let commit_id = CommitId::new();
        let mut source = ContinuityDb::new(FileKernel::open(&source_path)?);
        source.ingest_cells_at_with_commit_id(
            vec![sample_cell(
                "project:continuitydb:file-import-summary",
                0.91,
                12,
            )?],
            committed_at,
            commit_id,
        )?;
        source.export_commits_json_file(CommitManifestLookup::default(), &backup_path)?;
        let mut target = ContinuityDb::new(FileKernel::open(&target_path)?);

        let summary = target.import_commits_json_file_with_summary(&backup_path)?;

        assert_eq!(
            summary,
            CommitImportSummary {
                imported_commits: 1,
                next_after: Some(commit_id),
            }
        );

        fs::remove_file(source_path)?;
        fs::remove_file(target_path)?;
        fs::remove_file(backup_path)?;
        Ok(())
    }

    #[test]
    fn api_import_commit_backup_json_file_rejects_invalid_json(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let target_path = temp_file_kernel_path("continuitydb-api-import-backup-invalid-target");
        let backup_path = temp_file_kernel_path("continuitydb-api-import-backup-invalid-file");
        std::fs::write(&backup_path, "{not valid json}\n")?;
        let mut target = ContinuityDb::new(FileKernel::open(&target_path)?);

        let result = target.import_commits_json_file(&backup_path);

        assert!(matches!(result, Err(ContinuityError::CommitExportJson)));

        std::fs::remove_file(target_path)?;
        std::fs::remove_file(backup_path)?;
        Ok(())
    }

    #[test]
    fn api_export_commit_backup_json_file_reports_io_failure(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let source_path = temp_file_kernel_path("continuitydb-api-export-backup-io-source");
        let missing_dir = std::env::temp_dir().join(format!(
            "continuitydb-api-missing-dir-{:?}",
            StateCellId::new()
        ));
        let backup_path = missing_dir.join("backup.json");
        let committed_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let commit_id = CommitId::new();
        let mut db = ContinuityDb::new(FileKernel::open(&source_path)?);
        db.ingest_cells_at_with_commit_id(
            vec![sample_cell(
                "project:continuitydb:file-export-io",
                0.91,
                12,
            )?],
            committed_at,
            commit_id,
        )?;

        let result = db.export_commits_json_file(CommitManifestLookup::default(), backup_path);

        assert!(matches!(result, Err(ContinuityError::CommitExportFileIo)));

        std::fs::remove_file(source_path)?;
        Ok(())
    }

    #[test]
    fn api_imports_exported_commit_batch() -> Result<(), Box<dyn std::error::Error>> {
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
        let mut source = ContinuityDb::new(MemoryKernel::default());
        source.ingest_cells_at_with_commit_id(
            vec![sample_cell("project:continuitydb:import-first", 0.91, 12)?],
            first_time,
            first_commit,
        )?;
        source.ingest_cells_at_with_commit_id(
            vec![
                sample_cell("project:continuitydb:import-second-a", 0.83, 15)?,
                sample_cell("project:continuitydb:import-second-b", 0.82, 16)?,
            ],
            second_time,
            second_commit,
        )?;
        let batch = source.export_commits(CommitManifestLookup::default())?;
        let mut target = ContinuityDb::new(MemoryKernel::default());

        let imported = target.import_commit_batch(batch.clone())?;

        assert_eq!(imported, 2);
        assert_eq!(
            target.export_commits(CommitManifestLookup::default())?,
            batch
        );
        Ok(())
    }

    #[test]
    fn api_import_commit_batch_summary_reports_count_and_cursor(
    ) -> Result<(), Box<dyn std::error::Error>> {
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
        let mut source = ContinuityDb::new(MemoryKernel::default());
        source.ingest_cells_at_with_commit_id(
            vec![sample_cell("project:continuitydb:summary-first", 0.91, 12)?],
            first_time,
            first_commit,
        )?;
        source.ingest_cells_at_with_commit_id(
            vec![sample_cell(
                "project:continuitydb:summary-second",
                0.83,
                15,
            )?],
            second_time,
            second_commit,
        )?;
        let batch = source.export_commits(CommitManifestLookup {
            after: None,
            limit: Some(1),
        })?;
        let mut target = ContinuityDb::new(MemoryKernel::default());

        let summary = target.import_commit_batch_with_summary(batch.clone())?;

        assert_eq!(
            summary,
            CommitImportSummary {
                imported_commits: 1,
                next_after: Some(first_commit),
            }
        );
        assert_eq!(
            target.export_commits(CommitManifestLookup::default())?,
            batch
        );
        Ok(())
    }

    #[test]
    fn api_import_commit_batch_count_delegates_to_summary() -> Result<(), Box<dyn std::error::Error>>
    {
        let committed_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let commit_id = CommitId::new();
        let mut source = ContinuityDb::new(MemoryKernel::default());
        source.ingest_cells_at_with_commit_id(
            vec![sample_cell("project:continuitydb:summary-count", 0.91, 12)?],
            committed_at,
            commit_id,
        )?;
        let batch = source.export_commits(CommitManifestLookup::default())?;
        let mut target = ContinuityDb::new(MemoryKernel::default());

        let imported = target.import_commit_batch(batch)?;

        assert_eq!(imported, 1);
        Ok(())
    }

    #[test]
    fn api_copies_commit_page_from_source_database() -> Result<(), Box<dyn std::error::Error>> {
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
        let mut source = ContinuityDb::new(MemoryKernel::default());
        source.ingest_cells_at_with_commit_id(
            vec![sample_cell("project:continuitydb:copy-first", 0.91, 12)?],
            first_time,
            first_commit,
        )?;
        source.ingest_cells_at_with_commit_id(
            vec![sample_cell("project:continuitydb:copy-second", 0.83, 15)?],
            second_time,
            second_commit,
        )?;
        let mut target = ContinuityDb::new(MemoryKernel::default());

        let summary = target.copy_commits_from(
            &source,
            CommitManifestLookup {
                after: None,
                limit: Some(1),
            },
        )?;

        assert_eq!(
            summary,
            CommitImportSummary {
                imported_commits: 1,
                next_after: Some(first_commit),
            }
        );
        assert_eq!(target.commit_manifests()?.len(), 1);
        assert!(target.commit_manifest(first_commit)?.is_some());
        assert!(target.commit_manifest(second_commit)?.is_none());
        Ok(())
    }

    #[test]
    fn api_copies_next_commit_page_from_source_database() -> Result<(), Box<dyn std::error::Error>>
    {
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
        let mut source = ContinuityDb::new(MemoryKernel::default());
        source.ingest_cells_at_with_commit_id(
            vec![sample_cell(
                "project:continuitydb:copy-next-first",
                0.91,
                12,
            )?],
            first_time,
            first_commit,
        )?;
        source.ingest_cells_at_with_commit_id(
            vec![sample_cell(
                "project:continuitydb:copy-next-second",
                0.83,
                15,
            )?],
            second_time,
            second_commit,
        )?;
        let mut target = ContinuityDb::new(MemoryKernel::default());

        let summary = target.copy_commits_from(
            &source,
            CommitManifestLookup {
                after: Some(first_commit),
                limit: Some(1),
            },
        )?;

        assert_eq!(
            summary,
            CommitImportSummary {
                imported_commits: 1,
                next_after: Some(second_commit),
            }
        );
        assert_eq!(target.commit_manifests()?.len(), 1);
        assert!(target.commit_manifest(first_commit)?.is_none());
        assert!(target.commit_manifest(second_commit)?.is_some());
        Ok(())
    }

    #[test]
    fn api_copy_commits_from_empty_source_returns_empty_summary(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let source = ContinuityDb::new(MemoryKernel::default());
        let mut target = ContinuityDb::new(MemoryKernel::default());

        let summary = target.copy_commits_from(&source, CommitManifestLookup::default())?;

        assert_eq!(
            summary,
            CommitImportSummary {
                imported_commits: 0,
                next_after: None,
            }
        );
        assert!(target.commit_manifests()?.is_empty());
        Ok(())
    }

    #[test]
    fn api_copy_commits_rejects_duplicate_target_commit_without_mutation(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let committed_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let commit_id = CommitId::new();
        let mut source = ContinuityDb::new(MemoryKernel::default());
        source.ingest_cells_at_with_commit_id(
            vec![sample_cell(
                "project:continuitydb:copy-duplicate-source",
                0.91,
                12,
            )?],
            committed_at,
            commit_id,
        )?;
        let mut target = ContinuityDb::new(MemoryKernel::default());
        target.ingest_cells_at_with_commit_id(
            vec![sample_cell(
                "project:continuitydb:copy-duplicate-target",
                0.83,
                15,
            )?],
            committed_at,
            commit_id,
        )?;

        let result = target.copy_commits_from(&source, CommitManifestLookup::default());

        assert!(matches!(
            result,
            Err(ContinuityError::Kernel(KernelError::DuplicateCommit))
        ));
        assert_eq!(target.commit_manifests()?.len(), 1);
        Ok(())
    }

    #[test]
    fn api_copy_commits_reports_unknown_source_cursor() -> Result<(), Box<dyn std::error::Error>> {
        let source = ContinuityDb::new(MemoryKernel::default());
        let mut target = ContinuityDb::new(MemoryKernel::default());

        let result = target.copy_commits_from(
            &source,
            CommitManifestLookup {
                after: Some(CommitId::new()),
                limit: Some(1),
            },
        );

        assert!(matches!(
            result,
            Err(ContinuityError::Kernel(KernelError::CommitNotFound))
        ));
        assert!(target.commit_manifests()?.is_empty());
        Ok(())
    }

    #[test]
    fn api_imports_empty_commit_batch_without_mutation() -> Result<(), Box<dyn std::error::Error>> {
        let mut target = ContinuityDb::new(MemoryKernel::default());

        let imported = target.import_commit_batch(CommitExportBatch {
            slices: Vec::new(),
            next_after: None,
        })?;

        assert_eq!(imported, 0);
        assert!(target
            .commit_slices(CommitManifestLookup::default())?
            .is_empty());
        Ok(())
    }

    #[test]
    fn api_import_rejects_malformed_batch_without_mutation(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let committed_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let commit_id = CommitId::new();
        let mut cell = sample_cell("project:continuitydb:malformed-import", 0.91, 12)?;
        cell.commit_id = commit_id;
        cell.system_time = continuitydb_core::SystemTimeRange::open_from(committed_at);
        let manifest = continuitydb_core::CommitManifest::new(
            commit_id,
            committed_at,
            vec![StateCellId::new()],
        );
        let mut target = ContinuityDb::new(MemoryKernel::default());

        let result = target.import_commit_batch(CommitExportBatch {
            slices: vec![CommitSlice {
                manifest,
                cells: vec![cell],
            }],
            next_after: Some(commit_id),
        });

        assert!(matches!(
            result,
            Err(ContinuityError::InvalidCommitExport { commit_id: rejected }) if rejected == commit_id
        ));
        assert!(target
            .commit_slices(CommitManifestLookup::default())?
            .is_empty());
        Ok(())
    }

    #[test]
    fn api_import_rejects_existing_target_commit_without_mutation(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let committed_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let commit_id = CommitId::new();
        let mut source = ContinuityDb::new(MemoryKernel::default());
        source.ingest_cells_at_with_commit_id(
            vec![sample_cell(
                "project:continuitydb:existing-source",
                0.91,
                12,
            )?],
            committed_at,
            commit_id,
        )?;
        let batch = source.export_commits(CommitManifestLookup::default())?;
        let mut target = ContinuityDb::new(MemoryKernel::default());
        target.ingest_cells_at_with_commit_id(
            vec![sample_cell(
                "project:continuitydb:existing-target",
                0.83,
                15,
            )?],
            committed_at,
            commit_id,
        )?;

        let result = target.import_commit_batch(batch);

        assert!(matches!(
            result,
            Err(ContinuityError::Kernel(KernelError::DuplicateCommit))
        ));
        assert_eq!(
            target.commit_slices(CommitManifestLookup::default())?.len(),
            1
        );
        Ok(())
    }

    #[test]
    fn api_compacts_file_store_to_canonical_records() -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_file_kernel_path("continuitydb-api-file-compact");
        let committed_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let commit_id = CommitId::new();
        let cell = sample_cell("project:continuitydb:api-file-compact", 0.91, 12)?;
        let cell_id = cell.id;
        let mut db = ContinuityDb::new(FileKernel::open(&path)?);
        db.ingest_cell_at_with_commit_id(cell, committed_at, commit_id)?;

        db.compact_file_store()?;

        let records = std::fs::read_to_string(&path)?
            .lines()
            .map(str::to_string)
            .collect::<Vec<_>>();
        assert_eq!(records.len(), 3);
        assert!(records[0].contains(r#""type":"header""#));
        assert!(records[1].contains(r#""type":"cell""#));
        assert!(records[1].contains(r#""checksum":"continuitydb-fnv1a64:"#));
        assert!(records[2].contains(r#""type":"commit""#));
        assert!(records[2].contains(r#""checksum":"continuitydb-fnv1a64:"#));
        assert_eq!(db.commit_cells(commit_id)?[0].id, cell_id);
        std::fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn api_skips_file_compaction_when_store_is_canonical() -> Result<(), Box<dyn std::error::Error>>
    {
        let path = temp_file_kernel_path("api-file-compact-if-needed-canonical");
        let mut db = ContinuityDb::open_file(&path)?;

        let summary = db.compact_file_store_if_needed()?;

        assert!(!summary.compacted);
        assert_eq!(summary.before, summary.after);
        assert!(!summary.after.compaction_recommended);

        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn api_compacts_file_store_when_health_recommends_it() -> Result<(), Box<dyn std::error::Error>>
    {
        let path = temp_file_kernel_path("api-file-compact-if-needed-legacy");
        write_legacy_file_store(&path)?;
        let mut db = ContinuityDb::open_file(&path)?;

        let summary = db.compact_file_store_if_needed()?;

        assert!(summary.compacted);
        assert!(summary.before.compaction_recommended);
        assert!(!summary.after.compaction_recommended);
        assert_eq!(summary.after.legacy_raw_cells, 0);
        assert!(summary.after.has_header);

        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn api_file_compaction_preserves_commit_slices_after_reopen(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_file_kernel_path("continuitydb-api-file-compact-slices");
        let committed_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let commit_id = CommitId::new();
        let first = sample_cell("project:continuitydb:api-file-compact-first", 0.91, 12)?;
        let second = sample_cell("project:continuitydb:api-file-compact-second", 0.83, 15)?;
        let expected_ids = vec![first.id, second.id];
        {
            let mut db = ContinuityDb::new(FileKernel::open(&path)?);
            db.ingest_cells_at_with_commit_id(vec![first, second], committed_at, commit_id)?;
            db.compact_file_store()?;
        }

        let reopened = ContinuityDb::new(FileKernel::open(&path)?);
        let slices = reopened.commit_slices(CommitManifestLookup::default())?;

        assert_eq!(slices.len(), 1);
        assert_eq!(slices[0].manifest.commit_id, commit_id);
        assert_eq!(
            slices[0]
                .cells
                .iter()
                .map(|cell| cell.id)
                .collect::<Vec<_>>(),
            expected_ids
        );
        std::fs::remove_file(path)?;
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

    #[cfg(feature = "steward")]
    #[test]
    fn api_records_steward_proposal_audit_to_kernel() -> Result<(), Box<dyn std::error::Error>> {
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let proposal = sample_steward_proposal("New evidence supersedes the prior cell.")?;
        let proposal_id = proposal.id();

        let record =
            db.record_steward_proposal(proposal, &ProposalPolicy::strict(), test_steward_time()?)?;

        assert_eq!(record.proposal().id(), proposal_id);
        assert_eq!(record.decision().outcome(), ProposalOutcome::Accepted);
        let records = db.steward_proposal_records()?;
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].proposal().id(), proposal_id);
        let by_id = db.steward_proposal_record(proposal_id)?;
        assert_eq!(
            by_id.map(|record| record.proposal().id()),
            Some(proposal_id)
        );
        Ok(())
    }

    #[cfg(feature = "steward")]
    #[test]
    fn api_records_rejected_steward_proposal_audit() -> Result<(), Box<dyn std::error::Error>> {
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let proposal = StewardProposal::new(
            ProposalId::new(),
            test_steward_identity()?,
            StewardAction::AdjustConfidence {
                cell_id: StateCellId::new(),
                proposed_confidence: 1.5,
            },
            "Confidence adjustment is outside policy bounds.",
            vec!["test://steward".to_string()],
            test_steward_time()?,
        )?;

        let record =
            db.record_steward_proposal(proposal, &ProposalPolicy::strict(), test_steward_time()?)?;

        assert_eq!(record.decision().outcome(), ProposalOutcome::Rejected);
        assert_eq!(
            record.decision().reasons(),
            &["policy:invalid-confidence".to_string()]
        );
        Ok(())
    }

    #[cfg(feature = "steward")]
    #[test]
    fn api_audits_conflict_resolutions_with_steward() -> Result<(), Box<dyn std::error::Error>> {
        let committed_at = test_steward_time()?;
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let low = sample_cell_with_payload_day_and_confidence(
            "project:continuitydb:steward-conflict",
            "release is blocked",
            20,
            0.55,
            12,
        )?;
        let high = sample_cell_with_payload_day_and_confidence(
            "project:continuitydb:steward-conflict",
            "release is ready",
            21,
            0.95,
            12,
        )?;
        let low_id = db.ingest_cell_at(low, committed_at)?;
        let high_id = db.ingest_cell_at(high, committed_at)?;
        let steward = test_conflict_steward()?;

        let audit = db.audit_conflict_resolutions_with_steward(
            [low_id, high_id],
            &steward,
            &ProposalPolicy::strict(),
            committed_at,
        )?;

        assert_eq!(audit.scan.recommendations.len(), 1);
        assert_eq!(audit.proposals.len(), 1);
        assert_eq!(audit.records.len(), 1);
        assert_eq!(audit.records[0].proposal().id(), audit.proposals[0].id());
        assert_eq!(
            audit.records[0].decision().outcome(),
            ProposalOutcome::Accepted
        );
        assert_eq!(
            audit.proposals[0].action(),
            &StewardAction::LinkRevision {
                source: high_id,
                kind: RevisionLinkKind::Supersedes,
                target: low_id,
            }
        );
        assert_eq!(db.steward_proposal_records()?.len(), 1);
        Ok(())
    }

    #[cfg(feature = "steward")]
    #[test]
    fn api_steward_conflict_audit_allows_empty_and_singleton_inputs(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let committed_at = test_steward_time()?;
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let cell = sample_cell_with_payload_day_and_confidence(
            "project:continuitydb:steward-singleton",
            "release is ready",
            20,
            0.95,
            12,
        )?;
        let cell_id = db.ingest_cell_at(cell, committed_at)?;
        let steward = test_conflict_steward()?;

        let empty = db.audit_conflict_resolutions_with_steward(
            [],
            &steward,
            &ProposalPolicy::strict(),
            committed_at,
        )?;
        let singleton = db.audit_conflict_resolutions_with_steward(
            [cell_id],
            &steward,
            &ProposalPolicy::strict(),
            committed_at,
        )?;

        assert!(empty.scan.recommendations.is_empty());
        assert!(empty.proposals.is_empty());
        assert!(empty.records.is_empty());
        assert!(singleton.scan.recommendations.is_empty());
        assert!(singleton.proposals.is_empty());
        assert!(singleton.records.is_empty());
        assert!(db.steward_proposal_records()?.is_empty());
        Ok(())
    }

    #[cfg(feature = "steward")]
    #[test]
    fn api_steward_conflict_audit_reports_missing_id_without_audit(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let committed_at = test_steward_time()?;
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let cell = sample_cell_with_payload_day_and_confidence(
            "project:continuitydb:steward-missing",
            "release is ready",
            20,
            0.95,
            12,
        )?;
        let cell_id = db.ingest_cell_at(cell, committed_at)?;
        let missing_id = StateCellId::new();
        let steward = test_conflict_steward()?;

        let result = db.audit_conflict_resolutions_with_steward(
            [cell_id, missing_id],
            &steward,
            &ProposalPolicy::strict(),
            committed_at,
        );

        assert!(matches!(
            result,
            Err(ContinuityError::CellNotFound { cell_id }) if cell_id == missing_id
        ));
        assert!(db.steward_proposal_records()?.is_empty());
        Ok(())
    }

    #[cfg(feature = "steward")]
    #[test]
    fn api_audits_subscribed_frontier_watch_with_steward() -> Result<(), Box<dyn std::error::Error>>
    {
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let cell_id = StateCellId::new();
        let runner = test_frontier_runner(cell_id, FrontierWatchSignal::StaleEvidence)?;
        let event = FrontierWatchEvent::new(
            cell_id,
            FrontierWatchSignal::StaleEvidence,
            "test://frontier-stale",
            test_steward_time()?,
        );

        let audit = db.audit_frontier_watch_with_steward(
            &runner,
            vec![event],
            &ProposalPolicy::strict(),
            test_steward_time()?,
        )?;

        assert_eq!(audit.proposals.len(), 1);
        assert_eq!(audit.records.len(), 1);
        assert_eq!(audit.records[0].proposal().id(), audit.proposals[0].id());
        assert_eq!(
            audit.records[0].decision().outcome(),
            ProposalOutcome::Accepted
        );
        assert_eq!(
            audit.proposals[0].action(),
            &StewardAction::RequestVerification {
                cell_id: Some(cell_id),
                request: "Refresh stale evidence for frontier cell.".to_string(),
            }
        );
        assert_eq!(db.steward_proposal_records()?.len(), 1);
        Ok(())
    }

    #[cfg(feature = "steward")]
    #[test]
    fn api_frontier_watch_audit_ignores_unsubscribed_and_benign_events(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let cell_id = StateCellId::new();
        let runner = test_frontier_runner(cell_id, FrontierWatchSignal::StaleEvidence)?;
        let events = vec![
            FrontierWatchEvent::new(
                StateCellId::new(),
                FrontierWatchSignal::StaleEvidence,
                "test://frontier-other-cell",
                test_steward_time()?,
            ),
            FrontierWatchEvent::new(
                cell_id,
                FrontierWatchSignal::Benign,
                "test://frontier-benign",
                test_steward_time()?,
            ),
        ];

        let audit = db.audit_frontier_watch_with_steward(
            &runner,
            events,
            &ProposalPolicy::strict(),
            test_steward_time()?,
        )?;

        assert!(audit.proposals.is_empty());
        assert!(audit.records.is_empty());
        assert!(db.steward_proposal_records()?.is_empty());
        Ok(())
    }

    #[cfg(feature = "steward")]
    #[test]
    fn api_frontier_watch_audit_deduplicates_multiple_matching_subscriptions(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let cell_id = StateCellId::new();
        let mut store = MemoryFrontierSubscriptionStore::default();
        for citation in [
            "test://frontier-subscription-1",
            "test://frontier-subscription-2",
        ] {
            store.append_subscription(FrontierSubscription::new(
                FrontierSubscriptionId::new(),
                cell_id,
                vec![FrontierWatchSignal::HighImpactUncertainty],
                citation,
                test_steward_time()?,
            )?)?;
        }
        let runner =
            FrontierSubscriptionRunner::new(FrontierSteward::new(test_steward_identity()?), store);

        let audit = db.audit_frontier_watch_with_steward(
            &runner,
            vec![FrontierWatchEvent::new(
                cell_id,
                FrontierWatchSignal::HighImpactUncertainty,
                "test://frontier-uncertain",
                test_steward_time()?,
            )],
            &ProposalPolicy::strict(),
            test_steward_time()?,
        )?;

        assert_eq!(audit.proposals.len(), 1);
        assert_eq!(audit.records.len(), 1);
        assert_eq!(db.steward_proposal_records()?.len(), 1);
        Ok(())
    }

    #[cfg(feature = "steward")]
    #[test]
    fn api_mark_frontier_application_appends_successor() -> Result<(), Box<dyn std::error::Error>> {
        let initial_commit = test_steward_time()?;
        let apply_commit = Utc
            .with_ymd_and_hms(2026, 5, 20, 13, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let original_id = db.ingest_cell_at(
            sample_cell("project:continuitydb:apply-frontier", 0.91, 12)?,
            initial_commit,
        )?;
        let proposal = StewardProposal::new(
            ProposalId::new(),
            test_steward_identity()?,
            StewardAction::MarkFrontier {
                cell_id: original_id,
            },
            "Stale evidence should move this cell to frontier monitoring.",
            vec!["test://frontier-apply".to_string()],
            initial_commit,
        )?;
        let record =
            db.record_steward_proposal(proposal, &ProposalPolicy::strict(), initial_commit)?;

        let successor_id = db
            .apply_accepted_mark_frontier_proposal_at(&record, apply_commit)?
            .ok_or_else(|| std::io::Error::other("expected successor"))?;

        let original = db
            .kernel()
            .lookup_cells(CellLookup {
                cell_id: Some(original_id),
                ..CellLookup::default()
            })?
            .into_iter()
            .next()
            .ok_or_else(|| std::io::Error::other("missing original"))?;
        let successor = db
            .kernel()
            .lookup_cells(CellLookup {
                cell_id: Some(successor_id),
                ..CellLookup::default()
            })?
            .into_iter()
            .next()
            .ok_or_else(|| std::io::Error::other("missing successor"))?;

        assert_ne!(successor_id, original_id);
        assert_eq!(original.activation, ActivationState::Active);
        assert_eq!(successor.activation, ActivationState::Frontier);
        assert_eq!(successor.system_time.from(), apply_commit);
        Ok(())
    }

    #[cfg(feature = "steward")]
    #[test]
    fn api_mark_frontier_application_ignores_rejected_record(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let committed_at = test_steward_time()?;
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let original_id = db.ingest_cell_at(
            sample_cell("project:continuitydb:rejected-frontier", 0.91, 12)?,
            committed_at,
        )?;
        let proposal = StewardProposal::new(
            ProposalId::new(),
            test_steward_identity()?,
            StewardAction::MarkFrontier {
                cell_id: original_id,
            },
            "Policy rejected this frontier application.",
            vec!["test://frontier-rejected".to_string()],
            committed_at,
        )?;
        let decision = continuitydb_steward::ProposalDecision::new(
            proposal.id(),
            ProposalOutcome::Rejected,
            vec!["policy:test-rejected".to_string()],
            committed_at,
        );
        let record = continuitydb_steward::ProposalAuditRecord::new(proposal, decision)?;

        let applied = db.apply_accepted_mark_frontier_proposal_at(&record, committed_at)?;

        assert_eq!(applied, None);
        assert_eq!(db.kernel().lookup_cells(CellLookup::default())?.len(), 1);
        Ok(())
    }

    #[cfg(feature = "steward")]
    #[test]
    fn api_mark_frontier_application_rejects_unsupported_accepted_action(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let committed_at = test_steward_time()?;
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let proposal = StewardProposal::new(
            ProposalId::new(),
            test_steward_identity()?,
            StewardAction::LinkRevision {
                source: StateCellId::new(),
                kind: RevisionLinkKind::Supersedes,
                target: StateCellId::new(),
            },
            "Supersession application is not implemented in this slice.",
            vec!["test://unsupported-apply".to_string()],
            committed_at,
        )?;
        let record =
            db.record_steward_proposal(proposal, &ProposalPolicy::strict(), committed_at)?;

        let result = db.apply_accepted_mark_frontier_proposal_at(&record, committed_at);

        assert!(matches!(
            result,
            Err(ContinuityError::UnsupportedStewardProposalAction)
        ));
        Ok(())
    }

    #[cfg(feature = "steward")]
    #[test]
    fn api_mark_frontier_application_reports_missing_cell() -> Result<(), Box<dyn std::error::Error>>
    {
        let committed_at = test_steward_time()?;
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let missing_id = StateCellId::new();
        let proposal = StewardProposal::new(
            ProposalId::new(),
            test_steward_identity()?,
            StewardAction::MarkFrontier {
                cell_id: missing_id,
            },
            "Missing target should be reported before application.",
            vec!["test://frontier-missing".to_string()],
            committed_at,
        )?;
        let record =
            db.record_steward_proposal(proposal, &ProposalPolicy::strict(), committed_at)?;

        let result = db.apply_accepted_mark_frontier_proposal_at(&record, committed_at);

        assert!(matches!(
            result,
            Err(ContinuityError::CellNotFound { cell_id }) if cell_id == missing_id
        ));
        Ok(())
    }

    #[cfg(feature = "steward")]
    #[test]
    fn api_label_answerability_application_appends_successor(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let initial_commit = test_steward_time()?;
        let apply_commit = Utc
            .with_ymd_and_hms(2026, 5, 20, 13, 15, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let original_id = db.ingest_cell_at(
            sample_cell("project:continuitydb:apply-answerability", 0.91, 12)?,
            initial_commit,
        )?;
        let questions = vec![
            "what changed?".to_string(),
            "what needs review?".to_string(),
        ];
        let proposal = StewardProposal::new(
            ProposalId::new(),
            test_steward_identity()?,
            StewardAction::LabelAnswerability {
                cell_id: original_id,
                questions: questions.clone(),
            },
            "The cell can answer updated review questions.",
            vec!["test://answerability-apply".to_string()],
            initial_commit,
        )?;
        let record =
            db.record_steward_proposal(proposal, &ProposalPolicy::strict(), initial_commit)?;

        let successor_id = db
            .apply_accepted_label_answerability_proposal_at(&record, apply_commit)?
            .ok_or_else(|| std::io::Error::other("expected successor"))?;

        let original = db
            .kernel()
            .lookup_cells(CellLookup {
                cell_id: Some(original_id),
                ..CellLookup::default()
            })?
            .into_iter()
            .next()
            .ok_or_else(|| std::io::Error::other("missing original"))?;
        let successor = db
            .kernel()
            .lookup_cells(CellLookup {
                cell_id: Some(successor_id),
                ..CellLookup::default()
            })?
            .into_iter()
            .next()
            .ok_or_else(|| std::io::Error::other("missing successor"))?;

        assert_ne!(successor_id, original_id);
        assert_eq!(
            original.answerability.questions(),
            &["what should the agent know?".to_string()]
        );
        assert_eq!(successor.answerability.questions(), questions.as_slice());
        assert_eq!(successor.system_time.from(), apply_commit);
        Ok(())
    }

    #[cfg(feature = "steward")]
    #[test]
    fn api_label_answerability_application_ignores_rejected_record(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let committed_at = test_steward_time()?;
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let original_id = db.ingest_cell_at(
            sample_cell("project:continuitydb:rejected-answerability", 0.91, 12)?,
            committed_at,
        )?;
        let proposal = StewardProposal::new(
            ProposalId::new(),
            test_steward_identity()?,
            StewardAction::LabelAnswerability {
                cell_id: original_id,
                questions: vec!["what changed?".to_string()],
            },
            "Policy rejected this answerability application.",
            vec!["test://answerability-rejected".to_string()],
            committed_at,
        )?;
        let decision = continuitydb_steward::ProposalDecision::new(
            proposal.id(),
            ProposalOutcome::Rejected,
            vec!["policy:test-rejected".to_string()],
            committed_at,
        );
        let record = continuitydb_steward::ProposalAuditRecord::new(proposal, decision)?;

        let applied = db.apply_accepted_label_answerability_proposal_at(&record, committed_at)?;

        assert_eq!(applied, None);
        assert_eq!(db.kernel().lookup_cells(CellLookup::default())?.len(), 1);
        Ok(())
    }

    #[cfg(feature = "steward")]
    #[test]
    fn api_label_answerability_application_rejects_unsupported_accepted_action(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let committed_at = test_steward_time()?;
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let proposal = StewardProposal::new(
            ProposalId::new(),
            test_steward_identity()?,
            StewardAction::MarkFrontier {
                cell_id: StateCellId::new(),
            },
            "Frontier application belongs to a different method.",
            vec!["test://unsupported-answerability-apply".to_string()],
            committed_at,
        )?;
        let record =
            db.record_steward_proposal(proposal, &ProposalPolicy::strict(), committed_at)?;

        let result = db.apply_accepted_label_answerability_proposal_at(&record, committed_at);

        assert!(matches!(
            result,
            Err(ContinuityError::UnsupportedStewardProposalAction)
        ));
        Ok(())
    }

    #[cfg(feature = "steward")]
    #[test]
    fn api_label_answerability_application_reports_missing_cell(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let committed_at = test_steward_time()?;
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let missing_id = StateCellId::new();
        let proposal = StewardProposal::new(
            ProposalId::new(),
            test_steward_identity()?,
            StewardAction::LabelAnswerability {
                cell_id: missing_id,
                questions: vec!["what changed?".to_string()],
            },
            "Missing target should be reported before application.",
            vec!["test://answerability-missing".to_string()],
            committed_at,
        )?;
        let record =
            db.record_steward_proposal(proposal, &ProposalPolicy::strict(), committed_at)?;

        let result = db.apply_accepted_label_answerability_proposal_at(&record, committed_at);

        assert!(matches!(
            result,
            Err(ContinuityError::CellNotFound { cell_id }) if cell_id == missing_id
        ));
        Ok(())
    }

    #[cfg(feature = "steward")]
    #[test]
    fn api_adjust_confidence_application_appends_successor(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let initial_commit = test_steward_time()?;
        let apply_commit = Utc
            .with_ymd_and_hms(2026, 5, 20, 13, 30, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let original_id = db.ingest_cell_at(
            sample_cell("project:continuitydb:apply-confidence", 0.41, 12)?,
            initial_commit,
        )?;
        let proposal = StewardProposal::new(
            ProposalId::new(),
            test_steward_identity()?,
            StewardAction::AdjustConfidence {
                cell_id: original_id,
                proposed_confidence: 0.86,
            },
            "New evidence increases confidence.",
            vec!["test://confidence-apply".to_string()],
            initial_commit,
        )?;
        let record =
            db.record_steward_proposal(proposal, &ProposalPolicy::strict(), initial_commit)?;

        let successor_id = db
            .apply_accepted_adjust_confidence_proposal_at(&record, apply_commit)?
            .ok_or_else(|| std::io::Error::other("expected successor"))?;

        let original = db
            .kernel()
            .lookup_cells(CellLookup {
                cell_id: Some(original_id),
                ..CellLookup::default()
            })?
            .into_iter()
            .next()
            .ok_or_else(|| std::io::Error::other("missing original"))?;
        let successor = db
            .kernel()
            .lookup_cells(CellLookup {
                cell_id: Some(successor_id),
                ..CellLookup::default()
            })?
            .into_iter()
            .next()
            .ok_or_else(|| std::io::Error::other("missing successor"))?;

        assert_ne!(successor_id, original_id);
        assert_eq!(original.evidence[0].confidence, Confidence::new(0.41)?);
        assert_eq!(successor.evidence[0].confidence, Confidence::new(0.86)?);
        assert_eq!(successor.evidence[0].source, original.evidence[0].source);
        assert_eq!(
            successor.evidence[0].citation,
            original.evidence[0].citation
        );
        assert_eq!(successor.evidence[0].trust, original.evidence[0].trust);
        assert_eq!(successor.system_time.from(), apply_commit);
        Ok(())
    }

    #[cfg(feature = "steward")]
    #[test]
    fn api_adjust_confidence_application_ignores_rejected_record(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let committed_at = test_steward_time()?;
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let original_id = db.ingest_cell_at(
            sample_cell("project:continuitydb:rejected-confidence", 0.41, 12)?,
            committed_at,
        )?;
        let proposal = StewardProposal::new(
            ProposalId::new(),
            test_steward_identity()?,
            StewardAction::AdjustConfidence {
                cell_id: original_id,
                proposed_confidence: 0.86,
            },
            "Policy rejected this confidence application.",
            vec!["test://confidence-rejected".to_string()],
            committed_at,
        )?;
        let decision = continuitydb_steward::ProposalDecision::new(
            proposal.id(),
            ProposalOutcome::Rejected,
            vec!["policy:test-rejected".to_string()],
            committed_at,
        );
        let record = continuitydb_steward::ProposalAuditRecord::new(proposal, decision)?;

        let applied = db.apply_accepted_adjust_confidence_proposal_at(&record, committed_at)?;

        assert_eq!(applied, None);
        assert_eq!(db.kernel().lookup_cells(CellLookup::default())?.len(), 1);
        Ok(())
    }

    #[cfg(feature = "steward")]
    #[test]
    fn api_adjust_confidence_application_rejects_unsupported_accepted_action(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let committed_at = test_steward_time()?;
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let proposal = StewardProposal::new(
            ProposalId::new(),
            test_steward_identity()?,
            StewardAction::MarkFrontier {
                cell_id: StateCellId::new(),
            },
            "Frontier application belongs to a different method.",
            vec!["test://unsupported-confidence-apply".to_string()],
            committed_at,
        )?;
        let record =
            db.record_steward_proposal(proposal, &ProposalPolicy::strict(), committed_at)?;

        let result = db.apply_accepted_adjust_confidence_proposal_at(&record, committed_at);

        assert!(matches!(
            result,
            Err(ContinuityError::UnsupportedStewardProposalAction)
        ));
        Ok(())
    }

    #[cfg(feature = "steward")]
    #[test]
    fn api_adjust_confidence_application_reports_missing_cell(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let committed_at = test_steward_time()?;
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let missing_id = StateCellId::new();
        let proposal = StewardProposal::new(
            ProposalId::new(),
            test_steward_identity()?,
            StewardAction::AdjustConfidence {
                cell_id: missing_id,
                proposed_confidence: 0.86,
            },
            "Missing target should be reported before application.",
            vec!["test://confidence-missing".to_string()],
            committed_at,
        )?;
        let record =
            db.record_steward_proposal(proposal, &ProposalPolicy::strict(), committed_at)?;

        let result = db.apply_accepted_adjust_confidence_proposal_at(&record, committed_at);

        assert!(matches!(
            result,
            Err(ContinuityError::CellNotFound { cell_id }) if cell_id == missing_id
        ));
        Ok(())
    }

    #[cfg(feature = "steward")]
    #[test]
    fn api_request_verification_application_appends_targeted_work_cell(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let initial_commit = test_steward_time()?;
        let apply_commit = Utc
            .with_ymd_and_hms(2026, 5, 20, 13, 45, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let target_id = db.ingest_cell_at(
            sample_cell("project:continuitydb:verify-target", 0.51, 12)?,
            initial_commit,
        )?;
        let request = "Refresh the upstream citation and verify the latest observed state.";
        let proposal = StewardProposal::new(
            ProposalId::new(),
            test_steward_identity()?,
            StewardAction::RequestVerification {
                cell_id: Some(target_id),
                request: request.to_string(),
            },
            "The target cell has high-impact uncertainty.",
            vec!["test://verification-apply".to_string()],
            initial_commit,
        )?;
        let record =
            db.record_steward_proposal(proposal, &ProposalPolicy::strict(), initial_commit)?;

        let work_cell_id = db
            .apply_accepted_request_verification_proposal_at(&record, apply_commit)?
            .ok_or_else(|| std::io::Error::other("expected verification work cell"))?;

        let work_cell = db
            .kernel()
            .lookup_cells(CellLookup {
                cell_id: Some(work_cell_id),
                ..CellLookup::default()
            })?
            .into_iter()
            .next()
            .ok_or_else(|| std::io::Error::other("missing verification work cell"))?;
        let decoded_record: continuitydb_steward::ProposalAuditRecord =
            match work_cell.payload.clone() {
                CellPayload::Json(value) => serde_json::from_value(value)?,
                CellPayload::Text(_) | CellPayload::BlobRef(_) => {
                    return Err(std::io::Error::other("expected JSON payload").into())
                }
            };

        assert_eq!(work_cell.system_time.from(), apply_commit);
        assert_eq!(
            work_cell.valid_time.from(),
            apply_commit,
            "verification work becomes valid when deterministic application commits it"
        );
        assert_eq!(
            work_cell.answerability.questions(),
            &["what verification did the Steward request?".to_string()]
        );
        assert!(work_cell
            .anchors
            .iter()
            .any(|anchor| anchor.as_str() == "continuitydb:steward:verification-request"));
        assert!(work_cell.anchors.iter().any(|anchor| anchor.as_str()
            == format!("continuitydb:steward:verification-request:{target_id}")));
        assert_eq!(
            work_cell.evidence[0].source.as_str(),
            "continuitydb-steward"
        );
        assert_eq!(
            work_cell.evidence[0].citation.locator,
            "test://verification-apply"
        );
        assert_eq!(work_cell.evidence[0].confidence, Confidence::new(1.0)?);
        assert_eq!(work_cell.evidence[0].trust, vec![TrustSignal::Derived]);
        assert_eq!(
            work_cell.cost.token_count,
            request.split_whitespace().count() as i64
        );
        assert_eq!(decoded_record, record);
        assert_eq!(work_cell.dependencies.len(), 1);
        assert_eq!(work_cell.dependencies[0].target, target_id);
        assert_eq!(
            work_cell.dependencies[0].kind,
            CellDependencyKind::DependsOn
        );
        Ok(())
    }

    #[cfg(feature = "steward")]
    #[test]
    fn api_request_verification_application_appends_general_work_cell(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let committed_at = test_steward_time()?;
        let apply_commit = Utc
            .with_ymd_and_hms(2026, 5, 20, 14, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let proposal = StewardProposal::new(
            ProposalId::new(),
            test_steward_identity()?,
            StewardAction::RequestVerification {
                cell_id: None,
                request: "Check whether any frontier subscriptions need refreshed evidence."
                    .to_string(),
            },
            "The Steward requested general verification work.",
            vec!["test://verification-general".to_string()],
            committed_at,
        )?;
        let record = continuitydb_steward::ProposalAuditRecord::new(
            proposal.clone(),
            ProposalPolicy::strict().evaluate(&proposal, committed_at),
        )?;

        let work_cell_id = db
            .apply_accepted_request_verification_proposal_at(&record, apply_commit)?
            .ok_or_else(|| std::io::Error::other("expected verification work cell"))?;

        let work_cell = db
            .kernel()
            .lookup_cells(CellLookup {
                cell_id: Some(work_cell_id),
                ..CellLookup::default()
            })?
            .into_iter()
            .next()
            .ok_or_else(|| std::io::Error::other("missing verification work cell"))?;

        assert!(work_cell
            .anchors
            .iter()
            .any(|anchor| anchor.as_str() == "continuitydb:steward:verification-request:general"));
        assert!(work_cell.dependencies.is_empty());
        assert_eq!(work_cell.system_time.from(), apply_commit);
        Ok(())
    }

    #[cfg(feature = "steward")]
    #[test]
    fn api_request_verification_application_ignores_rejected_record(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let committed_at = test_steward_time()?;
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let target_id = db.ingest_cell_at(
            sample_cell("project:continuitydb:rejected-verification", 0.51, 12)?,
            committed_at,
        )?;
        let proposal = StewardProposal::new(
            ProposalId::new(),
            test_steward_identity()?,
            StewardAction::RequestVerification {
                cell_id: Some(target_id),
                request: "Rejected verification work should not be committed.".to_string(),
            },
            "Policy rejected this verification work.",
            vec!["test://verification-rejected".to_string()],
            committed_at,
        )?;
        let decision = continuitydb_steward::ProposalDecision::new(
            proposal.id(),
            ProposalOutcome::Rejected,
            vec!["policy:test-rejected".to_string()],
            committed_at,
        );
        let record = continuitydb_steward::ProposalAuditRecord::new(proposal, decision)?;

        let applied = db.apply_accepted_request_verification_proposal_at(&record, committed_at)?;

        assert_eq!(applied, None);
        assert_eq!(db.kernel().lookup_cells(CellLookup::default())?.len(), 1);
        Ok(())
    }

    #[cfg(feature = "steward")]
    #[test]
    fn api_request_verification_application_rejects_unsupported_accepted_action(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let committed_at = test_steward_time()?;
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let proposal = StewardProposal::new(
            ProposalId::new(),
            test_steward_identity()?,
            StewardAction::MarkFrontier {
                cell_id: StateCellId::new(),
            },
            "Frontier application belongs to a different method.",
            vec!["test://unsupported-verification-apply".to_string()],
            committed_at,
        )?;
        let record =
            db.record_steward_proposal(proposal, &ProposalPolicy::strict(), committed_at)?;

        let result = db.apply_accepted_request_verification_proposal_at(&record, committed_at);

        assert!(matches!(
            result,
            Err(ContinuityError::UnsupportedStewardProposalAction)
        ));
        Ok(())
    }

    #[cfg(feature = "steward")]
    #[test]
    fn api_request_verification_application_reports_missing_target_cell(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let committed_at = test_steward_time()?;
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let missing_id = StateCellId::new();
        let proposal = StewardProposal::new(
            ProposalId::new(),
            test_steward_identity()?,
            StewardAction::RequestVerification {
                cell_id: Some(missing_id),
                request: "Missing target should be reported before application.".to_string(),
            },
            "Missing target should be reported before application.",
            vec!["test://verification-missing".to_string()],
            committed_at,
        )?;
        let record =
            db.record_steward_proposal(proposal, &ProposalPolicy::strict(), committed_at)?;

        let result = db.apply_accepted_request_verification_proposal_at(&record, committed_at);

        assert!(matches!(
            result,
            Err(ContinuityError::CellNotFound { cell_id }) if cell_id == missing_id
        ));
        Ok(())
    }

    #[cfg(feature = "steward")]
    #[test]
    fn api_link_revision_application_appends_revision_link_cell(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let initial_commit = test_steward_time()?;
        let apply_commit = Utc
            .with_ymd_and_hms(2026, 5, 20, 14, 15, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let source_id = db.ingest_cell_at(
            sample_cell("project:continuitydb:revision-link-source", 0.91, 12)?,
            initial_commit,
        )?;
        let target_id = db.ingest_cell_at(
            sample_cell("project:continuitydb:revision-link-target", 0.41, 12)?,
            initial_commit,
        )?;
        let proposal = StewardProposal::new(
            ProposalId::new(),
            test_steward_identity()?,
            StewardAction::LinkRevision {
                source: source_id,
                kind: RevisionLinkKind::Supersedes,
                target: target_id,
            },
            "Accepted policy records that the stronger source supersedes the target.",
            vec!["test://revision-link-apply".to_string()],
            initial_commit,
        )?;
        let record =
            db.record_steward_proposal(proposal, &ProposalPolicy::strict(), initial_commit)?;

        let link_cell_id = db
            .apply_accepted_link_revision_proposal_at(&record, apply_commit)?
            .ok_or_else(|| std::io::Error::other("expected revision link cell"))?;

        let link_cell = db
            .kernel()
            .lookup_cells(CellLookup {
                cell_id: Some(link_cell_id),
                ..CellLookup::default()
            })?
            .into_iter()
            .next()
            .ok_or_else(|| std::io::Error::other("missing revision link cell"))?;
        let decoded_record: continuitydb_steward::ProposalAuditRecord =
            match link_cell.payload.clone() {
                CellPayload::Json(value) => serde_json::from_value(value)?,
                CellPayload::Text(_) | CellPayload::BlobRef(_) => {
                    return Err(std::io::Error::other("expected JSON payload").into())
                }
            };

        assert_eq!(link_cell.system_time.from(), apply_commit);
        assert_eq!(link_cell.valid_time.from(), apply_commit);
        assert_eq!(
            link_cell.answerability.questions(),
            &["what revision link did the Steward propose?".to_string()]
        );
        assert!(link_cell
            .anchors
            .iter()
            .any(|anchor| anchor.as_str() == "continuitydb:steward:revision-link"));
        assert!(link_cell.anchors.iter().any(|anchor| anchor.as_str()
            == format!("continuitydb:steward:revision-link:{source_id}:Supersedes:{target_id}")));
        assert_eq!(
            link_cell.evidence[0].source.as_str(),
            "continuitydb-steward"
        );
        assert_eq!(
            link_cell.evidence[0].citation.locator,
            "test://revision-link-apply"
        );
        assert_eq!(link_cell.evidence[0].confidence, Confidence::new(1.0)?);
        assert_eq!(link_cell.evidence[0].trust, vec![TrustSignal::Derived]);
        assert_eq!(decoded_record, record);
        assert_eq!(link_cell.dependencies.len(), 2);
        assert!(link_cell
            .dependencies
            .iter()
            .any(|dependency| dependency.target == source_id
                && dependency.kind == CellDependencyKind::DerivedFrom));
        assert!(link_cell
            .dependencies
            .iter()
            .any(|dependency| dependency.target == target_id
                && dependency.kind == CellDependencyKind::DerivedFrom));
        Ok(())
    }

    #[cfg(feature = "steward")]
    #[test]
    fn api_link_revision_application_ignores_rejected_record(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let committed_at = test_steward_time()?;
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let source_id = db.ingest_cell_at(
            sample_cell("project:continuitydb:rejected-link-source", 0.91, 12)?,
            committed_at,
        )?;
        let target_id = db.ingest_cell_at(
            sample_cell("project:continuitydb:rejected-link-target", 0.41, 12)?,
            committed_at,
        )?;
        let proposal = StewardProposal::new(
            ProposalId::new(),
            test_steward_identity()?,
            StewardAction::LinkRevision {
                source: source_id,
                kind: RevisionLinkKind::Supersedes,
                target: target_id,
            },
            "Policy rejected this revision link application.",
            vec!["test://revision-link-rejected".to_string()],
            committed_at,
        )?;
        let decision = continuitydb_steward::ProposalDecision::new(
            proposal.id(),
            ProposalOutcome::Rejected,
            vec!["policy:test-rejected".to_string()],
            committed_at,
        );
        let record = continuitydb_steward::ProposalAuditRecord::new(proposal, decision)?;

        let applied = db.apply_accepted_link_revision_proposal_at(&record, committed_at)?;

        assert_eq!(applied, None);
        assert_eq!(db.kernel().lookup_cells(CellLookup::default())?.len(), 2);
        Ok(())
    }

    #[cfg(feature = "steward")]
    #[test]
    fn api_link_revision_application_rejects_unsupported_accepted_action(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let committed_at = test_steward_time()?;
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let proposal = StewardProposal::new(
            ProposalId::new(),
            test_steward_identity()?,
            StewardAction::MarkFrontier {
                cell_id: StateCellId::new(),
            },
            "Frontier application belongs to a different method.",
            vec!["test://unsupported-link-apply".to_string()],
            committed_at,
        )?;
        let record =
            db.record_steward_proposal(proposal, &ProposalPolicy::strict(), committed_at)?;

        let result = db.apply_accepted_link_revision_proposal_at(&record, committed_at);

        assert!(matches!(
            result,
            Err(ContinuityError::UnsupportedStewardProposalAction)
        ));
        Ok(())
    }

    #[cfg(feature = "steward")]
    #[test]
    fn api_link_revision_application_reports_missing_source(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let committed_at = test_steward_time()?;
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let missing_source = StateCellId::new();
        let target_id = db.ingest_cell_at(
            sample_cell("project:continuitydb:missing-source-target", 0.41, 12)?,
            committed_at,
        )?;
        let proposal = StewardProposal::new(
            ProposalId::new(),
            test_steward_identity()?,
            StewardAction::LinkRevision {
                source: missing_source,
                kind: RevisionLinkKind::Supersedes,
                target: target_id,
            },
            "Missing source should be reported before application.",
            vec!["test://revision-link-missing-source".to_string()],
            committed_at,
        )?;
        let record =
            db.record_steward_proposal(proposal, &ProposalPolicy::strict(), committed_at)?;

        let result = db.apply_accepted_link_revision_proposal_at(&record, committed_at);

        assert!(matches!(
            result,
            Err(ContinuityError::CellNotFound { cell_id }) if cell_id == missing_source
        ));
        Ok(())
    }

    #[cfg(feature = "steward")]
    #[test]
    fn api_link_revision_application_reports_missing_target(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let committed_at = test_steward_time()?;
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let source_id = db.ingest_cell_at(
            sample_cell("project:continuitydb:missing-target-source", 0.91, 12)?,
            committed_at,
        )?;
        let missing_target = StateCellId::new();
        let proposal = StewardProposal::new(
            ProposalId::new(),
            test_steward_identity()?,
            StewardAction::LinkRevision {
                source: source_id,
                kind: RevisionLinkKind::Supersedes,
                target: missing_target,
            },
            "Missing target should be reported before application.",
            vec!["test://revision-link-missing-target".to_string()],
            committed_at,
        )?;
        let record =
            db.record_steward_proposal(proposal, &ProposalPolicy::strict(), committed_at)?;

        let result = db.apply_accepted_link_revision_proposal_at(&record, committed_at);

        assert!(matches!(
            result,
            Err(ContinuityError::CellNotFound { cell_id }) if cell_id == missing_target
        ));
        Ok(())
    }

    #[cfg(feature = "steward")]
    #[test]
    fn api_create_cell_draft_application_appends_state_cell(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let proposal_time = test_steward_time()?;
        let apply_commit = Utc
            .with_ymd_and_hms(2026, 5, 20, 14, 30, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let anchors = vec![
            SemanticAnchor::new("project:continuitydb:accepted-draft"),
            SemanticAnchor::new("project:continuitydb:accepted-draft:summary"),
        ];
        let payload_text = "ContinuityDB can promote accepted Steward drafts into StateCells.";
        let proposal = StewardProposal::new(
            ProposalId::new(),
            test_steward_identity()?,
            StewardAction::CreateCellDraft {
                anchors: anchors.clone(),
                payload_text: payload_text.to_string(),
            },
            "The cited evidence supports creating this StateCell.",
            vec![
                "test://create-cell-draft-1".to_string(),
                "test://create-cell-draft-2".to_string(),
            ],
            proposal_time,
        )?;
        let record =
            db.record_steward_proposal(proposal, &ProposalPolicy::strict(), proposal_time)?;

        let created_cell_id = db
            .apply_accepted_create_cell_draft_proposal_at(&record, apply_commit)?
            .ok_or_else(|| std::io::Error::other("expected created StateCell"))?;

        let created_cell = db
            .kernel()
            .lookup_cells(CellLookup {
                cell_id: Some(created_cell_id),
                ..CellLookup::default()
            })?
            .into_iter()
            .next()
            .ok_or_else(|| std::io::Error::other("missing created StateCell"))?;

        assert_eq!(created_cell.anchors, anchors);
        assert_eq!(created_cell.valid_time.from(), apply_commit);
        assert_eq!(created_cell.system_time.from(), apply_commit);
        assert_eq!(
            created_cell.answerability.questions(),
            &["what StateCell did the Steward draft?".to_string()]
        );
        assert_eq!(
            created_cell.payload,
            CellPayload::Text(payload_text.to_string())
        );
        assert_eq!(
            created_cell.cost.token_count,
            payload_text.split_whitespace().count() as i64
        );
        assert_eq!(created_cell.evidence.len(), 2);
        assert_eq!(
            created_cell.evidence[0].source.as_str(),
            "continuitydb-steward"
        );
        assert_eq!(
            created_cell.evidence[0].citation.locator,
            "test://create-cell-draft-1"
        );
        assert_eq!(created_cell.evidence[0].confidence, Confidence::new(1.0)?);
        assert_eq!(created_cell.evidence[0].trust, vec![TrustSignal::Derived]);
        assert_eq!(
            created_cell.evidence[1].citation.locator,
            "test://create-cell-draft-2"
        );
        Ok(())
    }

    #[cfg(feature = "steward")]
    #[test]
    fn api_create_cell_draft_application_ignores_rejected_record(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let committed_at = test_steward_time()?;
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let proposal = StewardProposal::new(
            ProposalId::new(),
            test_steward_identity()?,
            StewardAction::CreateCellDraft {
                anchors: vec![SemanticAnchor::new("project:continuitydb:rejected-draft")],
                payload_text: "Rejected draft should not become committed state.".to_string(),
            },
            "Policy rejected this draft.",
            vec!["test://create-cell-rejected".to_string()],
            committed_at,
        )?;
        let decision = continuitydb_steward::ProposalDecision::new(
            proposal.id(),
            ProposalOutcome::Rejected,
            vec!["policy:test-rejected".to_string()],
            committed_at,
        );
        let record = continuitydb_steward::ProposalAuditRecord::new(proposal, decision)?;

        let applied = db.apply_accepted_create_cell_draft_proposal_at(&record, committed_at)?;

        assert_eq!(applied, None);
        assert_eq!(db.kernel().lookup_cells(CellLookup::default())?.len(), 0);
        Ok(())
    }

    #[cfg(feature = "steward")]
    #[test]
    fn api_create_cell_draft_application_rejects_unsupported_accepted_action(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let committed_at = test_steward_time()?;
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let proposal = StewardProposal::new(
            ProposalId::new(),
            test_steward_identity()?,
            StewardAction::MarkFrontier {
                cell_id: StateCellId::new(),
            },
            "Frontier application belongs to a different method.",
            vec!["test://unsupported-create-cell-apply".to_string()],
            committed_at,
        )?;
        let record =
            db.record_steward_proposal(proposal, &ProposalPolicy::strict(), committed_at)?;

        let result = db.apply_accepted_create_cell_draft_proposal_at(&record, committed_at);

        assert!(matches!(
            result,
            Err(ContinuityError::UnsupportedStewardProposalAction)
        ));
        Ok(())
    }

    #[cfg(feature = "steward")]
    #[test]
    fn api_accepted_steward_proposal_dispatch_applies_create_cell_draft(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let proposal_time = test_steward_time()?;
        let apply_commit = Utc
            .with_ymd_and_hms(2026, 5, 20, 14, 45, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let anchor = SemanticAnchor::new("project:continuitydb:dispatch-draft");
        let proposal = StewardProposal::new(
            ProposalId::new(),
            test_steward_identity()?,
            StewardAction::CreateCellDraft {
                anchors: vec![anchor.clone()],
                payload_text: "Dispatcher can promote accepted draft proposals.".to_string(),
            },
            "The accepted proposal should dispatch to draft application.",
            vec!["test://dispatch-create-cell".to_string()],
            proposal_time,
        )?;
        let record =
            db.record_steward_proposal(proposal, &ProposalPolicy::strict(), proposal_time)?;

        let created_id = db
            .apply_accepted_steward_proposal_at(&record, apply_commit)?
            .ok_or_else(|| std::io::Error::other("expected created StateCell"))?;

        let created = db
            .kernel()
            .lookup_cells(CellLookup {
                cell_id: Some(created_id),
                ..CellLookup::default()
            })?
            .into_iter()
            .next()
            .ok_or_else(|| std::io::Error::other("missing created StateCell"))?;

        assert_eq!(created.anchors, vec![anchor]);
        assert_eq!(created.system_time.from(), apply_commit);
        Ok(())
    }

    #[cfg(feature = "steward")]
    #[test]
    fn api_accepted_steward_proposal_dispatch_applies_mark_frontier(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let proposal_time = test_steward_time()?;
        let apply_commit = Utc
            .with_ymd_and_hms(2026, 5, 20, 15, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let original_id = db.ingest_cell_at(
            sample_cell("project:continuitydb:dispatch-frontier", 0.91, 12)?,
            proposal_time,
        )?;
        let proposal = StewardProposal::new(
            ProposalId::new(),
            test_steward_identity()?,
            StewardAction::MarkFrontier {
                cell_id: original_id,
            },
            "The accepted proposal should dispatch to frontier application.",
            vec!["test://dispatch-frontier".to_string()],
            proposal_time,
        )?;
        let record =
            db.record_steward_proposal(proposal, &ProposalPolicy::strict(), proposal_time)?;

        let successor_id = db
            .apply_accepted_steward_proposal_at(&record, apply_commit)?
            .ok_or_else(|| std::io::Error::other("expected successor StateCell"))?;

        let successor = db
            .kernel()
            .lookup_cells(CellLookup {
                cell_id: Some(successor_id),
                ..CellLookup::default()
            })?
            .into_iter()
            .next()
            .ok_or_else(|| std::io::Error::other("missing successor StateCell"))?;

        assert_eq!(successor.activation, ActivationState::Frontier);
        assert_eq!(successor.system_time.from(), apply_commit);
        Ok(())
    }

    #[cfg(feature = "steward")]
    #[test]
    fn api_accepted_steward_proposal_dispatch_ignores_rejected_record(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let committed_at = test_steward_time()?;
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let proposal = StewardProposal::new(
            ProposalId::new(),
            test_steward_identity()?,
            StewardAction::CreateCellDraft {
                anchors: vec![SemanticAnchor::new(
                    "project:continuitydb:dispatch-rejected",
                )],
                payload_text: "Rejected dispatch should not create state.".to_string(),
            },
            "Policy rejected this dispatched proposal.",
            vec!["test://dispatch-rejected".to_string()],
            committed_at,
        )?;
        let decision = continuitydb_steward::ProposalDecision::new(
            proposal.id(),
            ProposalOutcome::Rejected,
            vec!["policy:test-rejected".to_string()],
            committed_at,
        );
        let record = continuitydb_steward::ProposalAuditRecord::new(proposal, decision)?;

        let applied = db.apply_accepted_steward_proposal_at(&record, committed_at)?;

        assert_eq!(applied, None);
        assert_eq!(db.kernel().lookup_cells(CellLookup::default())?.len(), 0);
        Ok(())
    }

    #[cfg(feature = "steward")]
    #[test]
    fn api_accepted_steward_proposal_dispatch_propagates_missing_cell(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let committed_at = test_steward_time()?;
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let missing_id = StateCellId::new();
        let proposal = StewardProposal::new(
            ProposalId::new(),
            test_steward_identity()?,
            StewardAction::MarkFrontier {
                cell_id: missing_id,
            },
            "Missing target should be reported by the delegated application method.",
            vec!["test://dispatch-missing".to_string()],
            committed_at,
        )?;
        let record =
            db.record_steward_proposal(proposal, &ProposalPolicy::strict(), committed_at)?;

        let result = db.apply_accepted_steward_proposal_at(&record, committed_at);

        assert!(matches!(
            result,
            Err(ContinuityError::CellNotFound { cell_id }) if cell_id == missing_id
        ));
        Ok(())
    }
}
