//! Storage kernel interface for ContinuityDB backends.

use chrono::{DateTime, Utc};
use continuitydb_core::{
    ActivationState, CellDependencyKind, CommitId, CommitManifest, Confidence, ContextGapKind,
    ContextPacketSelectionReason, ContextPacketStrategy, EpistemicAction, EpistemicActionReason,
    InvalidationConditionKind, LifecycleStage, MemoryProjectionKind, PromotionPolicy,
    RetentionPolicy, RevisionLinkKind, RevisionLinkRecord, Scope, StateCell, StateCellId,
    SystemTimeRange, UsePolicy,
};
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Read, Seek, SeekFrom, Write as IoWrite},
    path::{Path, PathBuf},
};
use thiserror::Error;

/// Errors produced by storage kernels.
#[derive(Debug, Error, PartialEq)]
pub enum KernelError {
    /// A duplicate immutable StateCell version was appended.
    #[error("state cell already exists")]
    DuplicateCell,
    /// A commit identifier already has a visible manifest.
    #[error("commit already exists")]
    DuplicateCommit,
    /// A duplicate immutable revision-link record was appended.
    #[error("revision link already exists")]
    DuplicateRevisionLink,
    /// Requested commit was not found.
    #[error("commit not found")]
    CommitNotFound,
    /// Storage kernel I/O failed.
    #[error("storage kernel I/O failed")]
    StoreIo,
    /// Storage kernel content could not be decoded.
    #[error("storage kernel content is corrupt")]
    StoreCorrupt,
    /// A specific JSONL file-kernel record is corrupt.
    #[error("storage kernel record at line {line} is corrupt")]
    StoreCorruptRecord {
        /// One-based physical line number in the JSONL log.
        line: usize,
    },
}

/// Query constraints supported by baseline storage kernels.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CellLookup {
    /// Optional immutable StateCell identifier filter.
    pub cell_id: Option<StateCellId>,
    /// Optional semantic anchor filter.
    pub semantic_anchor: Option<String>,
    /// Optional scope filter.
    pub scope: Option<Scope>,
    /// Optional valid-time as-of filter.
    pub valid_at: Option<DateTime<Utc>>,
    /// Optional system transaction-time as-of filter.
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
    /// Optional structural StateCell v2 packet selection reason filter.
    pub selection_reason: Option<ContextPacketSelectionReason>,
    /// Optional StateCell v2 trajectory-memory checkout strategy filter.
    pub trajectory_memory_strategy: Option<ContextPacketStrategy>,
    /// Optional minimum StateCell v2 trajectory-memory confidence filter.
    pub minimum_trajectory_memory_confidence: Option<Confidence>,
    /// Optional exact answerability question filter.
    pub answerability_question: Option<String>,
    /// Optional exact evidence-source filter.
    pub evidence_source: Option<String>,
    /// Optional minimum evidence confidence filter.
    pub minimum_confidence: Option<Confidence>,
    /// Optional dependency target filter.
    pub dependency_target: Option<StateCellId>,
    /// Optional dependency kind filter.
    pub dependency_kind: Option<CellDependencyKind>,
}

/// Query constraints for ordered commit manifest listing.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CommitManifestLookup {
    /// Exclusive cursor commit. When present, listing starts after this commit.
    pub after: Option<CommitId>,
    /// Maximum manifests to return.
    pub limit: Option<usize>,
}

/// Query constraints for revision-link record listing.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RevisionLinkLookup {
    /// Optional source StateCell version filter.
    pub source: Option<StateCellId>,
    /// Optional target StateCell version filter.
    pub target: Option<StateCellId>,
    /// Optional revision relationship kind filter.
    pub kind: Option<RevisionLinkKind>,
}

/// Broad durability class reported by a storage kernel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KernelDurability {
    /// In-process state with no durable persistence guarantee.
    Ephemeral,
    /// Durable append-log storage where indexes may be rebuilt from the log.
    AppendLog,
    /// Durable embedded storage with persistent indexes.
    IndexedEmbedded,
}

impl KernelDurability {
    const fn rank(self) -> u8 {
        match self {
            Self::Ephemeral => 0,
            Self::AppendLog => 1,
            Self::IndexedEmbedded => 2,
        }
    }

    const fn satisfies(self, minimum: Self) -> bool {
        self.rank() >= minimum.rank()
    }
}

/// Observable storage guarantees reported by a storage kernel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KernelCapabilities {
    /// Broad persistence level.
    pub durability: KernelDurability,
    /// Kernel writes immutable append-only records.
    pub append_only: bool,
    /// Kernel maintains derived in-process indexes.
    pub derived_indexes: bool,
    /// Kernel persists indexes durably rather than rebuilding them from the log.
    pub persistent_indexes: bool,
    /// Kernel stores explicit commit manifest records.
    pub explicit_commit_records: bool,
    /// Kernel flushes durable writes through the filesystem boundary.
    pub durable_flush: bool,
    /// Kernel can rewrite storage into a canonical compacted representation.
    pub compaction: bool,
}

/// Required storage guarantees for an embedder or operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KernelRequirements {
    /// Minimum acceptable persistence level.
    pub minimum_durability: KernelDurability,
    /// Requires immutable append-only writes.
    pub append_only: bool,
    /// Requires derived in-process indexes.
    pub derived_indexes: bool,
    /// Requires durable persistent indexes.
    pub persistent_indexes: bool,
    /// Requires explicit commit manifest records.
    pub explicit_commit_records: bool,
    /// Requires writes to flush through the filesystem boundary.
    pub durable_flush: bool,
    /// Requires storage compaction support.
    pub compaction: bool,
}

impl KernelRequirements {
    /// Requirements for correctness tests and temporary in-process stores.
    pub const fn ephemeral() -> Self {
        Self {
            minimum_durability: KernelDurability::Ephemeral,
            append_only: true,
            derived_indexes: false,
            persistent_indexes: false,
            explicit_commit_records: false,
            durable_flush: false,
            compaction: false,
        }
    }

    /// Requirements for durable local append-log storage.
    pub const fn durable_append_log() -> Self {
        Self {
            minimum_durability: KernelDurability::AppendLog,
            append_only: true,
            derived_indexes: false,
            persistent_indexes: false,
            explicit_commit_records: true,
            durable_flush: true,
            compaction: false,
        }
    }

    /// Requirements for durable append-log storage with persistent index checkpoints.
    pub const fn persistent_indexed_append_log() -> Self {
        Self {
            minimum_durability: KernelDurability::AppendLog,
            append_only: true,
            derived_indexes: true,
            persistent_indexes: true,
            explicit_commit_records: true,
            durable_flush: true,
            compaction: false,
        }
    }

    /// Requirements for future production indexed embedded storage.
    pub const fn indexed_embedded() -> Self {
        Self {
            minimum_durability: KernelDurability::IndexedEmbedded,
            append_only: true,
            derived_indexes: false,
            persistent_indexes: true,
            explicit_commit_records: true,
            durable_flush: true,
            compaction: false,
        }
    }
}

impl KernelCapabilities {
    /// Capabilities for in-process correctness kernels.
    pub const fn ephemeral() -> Self {
        Self {
            durability: KernelDurability::Ephemeral,
            append_only: true,
            derived_indexes: false,
            persistent_indexes: false,
            explicit_commit_records: false,
            durable_flush: false,
            compaction: false,
        }
    }

    /// Capabilities for the JSONL append-log file kernel.
    pub const fn file_append_log() -> Self {
        Self {
            durability: KernelDurability::AppendLog,
            append_only: true,
            derived_indexes: true,
            persistent_indexes: true,
            explicit_commit_records: true,
            durable_flush: true,
            compaction: true,
        }
    }

    /// Returns true when these capabilities meet all required guarantees.
    pub const fn satisfies(self, requirements: KernelRequirements) -> bool {
        self.durability.satisfies(requirements.minimum_durability)
            && (!requirements.append_only || self.append_only)
            && (!requirements.derived_indexes || self.derived_indexes)
            && (!requirements.persistent_indexes || self.persistent_indexes)
            && (!requirements.explicit_commit_records || self.explicit_commit_records)
            && (!requirements.durable_flush || self.durable_flush)
            && (!requirements.compaction || self.compaction)
    }
}

/// Minimal append and lookup contract required by the first ContinuityDB milestone.
pub trait StorageKernel {
    /// Returns the storage guarantees exposed by this kernel.
    fn capabilities(&self) -> KernelCapabilities {
        KernelCapabilities::ephemeral()
    }

    /// Appends an immutable StateCell version.
    fn append_cell(&mut self, cell: StateCell) -> Result<(), KernelError> {
        self.append_cell_at(cell, Utc::now())
    }

    /// Appends an immutable StateCell version at a deterministic system time.
    fn append_cell_at(
        &mut self,
        cell: StateCell,
        committed_at: DateTime<Utc>,
    ) -> Result<(), KernelError> {
        self.append_cell_at_with_commit_id(cell, committed_at, CommitId::new())
    }

    /// Appends an immutable StateCell version at a deterministic system time and commit ID.
    fn append_cell_at_with_commit_id(
        &mut self,
        cell: StateCell,
        committed_at: DateTime<Utc>,
        commit_id: CommitId,
    ) -> Result<(), KernelError> {
        self.append_cells_at_with_commit_id(std::iter::once(cell), committed_at, commit_id)
    }

    /// Appends immutable StateCell versions as one batch.
    fn append_cells<I>(&mut self, cells: I) -> Result<(), KernelError>
    where
        I: IntoIterator<Item = StateCell>,
    {
        self.append_cells_at(cells, Utc::now())
    }

    /// Appends immutable StateCell versions as one batch at a deterministic system time.
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

    /// Appends immutable StateCell versions as one batch at a deterministic system time and commit ID.
    fn append_cells_at_with_commit_id<I>(
        &mut self,
        cells: I,
        committed_at: DateTime<Utc>,
        commit_id: CommitId,
    ) -> Result<(), KernelError>
    where
        I: IntoIterator<Item = StateCell>;

    /// Looks up StateCells matching deterministic constraints.
    fn lookup_cells(&self, lookup: CellLookup) -> Result<Vec<StateCell>, KernelError>;

    /// Looks up a commit manifest by commit ID.
    fn lookup_commit_manifest(
        &self,
        commit_id: CommitId,
    ) -> Result<Option<CommitManifest>, KernelError>;

    /// Lists commit manifests in commit visibility order.
    fn list_commit_manifests(&self) -> Result<Vec<CommitManifest>, KernelError> {
        self.list_commit_manifests_matching(CommitManifestLookup::default())
    }

    /// Lists commit manifests matching deterministic constraints.
    fn list_commit_manifests_matching(
        &self,
        lookup: CommitManifestLookup,
    ) -> Result<Vec<CommitManifest>, KernelError>;

    /// Appends an immutable revision-link record.
    fn append_revision_link(
        &mut self,
        revision_link: RevisionLinkRecord,
    ) -> Result<(), KernelError>;

    /// Lists revision-link records matching deterministic constraints.
    fn list_revision_links(
        &self,
        lookup: RevisionLinkLookup,
    ) -> Result<Vec<RevisionLinkRecord>, KernelError>;
}

const FILE_KERNEL_FORMAT: &str = "continuitydb.file_kernel";
const FILE_KERNEL_FORMAT_VERSION: u32 = 1;
const FILE_KERNEL_INDEX_FORMAT: &str = "continuitydb.file_kernel.index";
const FILE_KERNEL_INDEX_VERSION: u32 = 12;
const FILE_KERNEL_CHECKSUM_ALGORITHM: &str = "continuitydb-fnv1a64";
const CONFIDENCE_INDEX_MICRO_UNITS: f32 = 1_000_000.0;
const FNV1A64_OFFSET: u64 = 0xcbf29ce484222325;
const FNV1A64_PRIME: u64 = 0x100000001b3;

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
struct FileKernelHeader {
    format: String,
    version: u32,
}

impl FileKernelHeader {
    fn current() -> Self {
        Self {
            format: FILE_KERNEL_FORMAT.to_string(),
            version: FILE_KERNEL_FORMAT_VERSION,
        }
    }

    fn validate(&self) -> Result<(), KernelError> {
        if self.format == FILE_KERNEL_FORMAT && self.version == FILE_KERNEL_FORMAT_VERSION {
            Ok(())
        } else {
            Err(KernelError::StoreCorrupt)
        }
    }
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum FileKernelRecord {
    Header {
        format: String,
        version: u32,
    },
    Cell {
        cell: Box<StateCell>,
        checksum: Option<String>,
    },
    Commit {
        manifest: CommitManifest,
        checksum: Option<String>,
    },
    RevisionLink {
        revision_link: RevisionLinkRecord,
        checksum: Option<String>,
    },
}

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
struct FileKernelPersistentIndexCheckpoint {
    format: String,
    version: u32,
    log_size_bytes: u64,
    cell_record_count: usize,
    commit_manifest_record_count: usize,
    revision_link_record_count: usize,
    semantic_anchor_address_count: usize,
    commit_address_count: usize,
    scope_address_count: usize,
    activation_address_count: usize,
    lifecycle_stage_address_count: usize,
    retention_policy_address_count: usize,
    use_policy_address_count: usize,
    promotion_policy_address_count: usize,
    projection_kind_address_count: usize,
    native_uncertainty_address_count: usize,
    native_surprise_bits_address_count: usize,
    native_salience_address_count: usize,
    native_context_affordance_address_count: usize,
    native_epistemic_pressure_address_count: usize,
    trajectory_memory_strategy_address_count: usize,
    trajectory_memory_confidence_address_count: usize,
    context_gap_kind_address_count: usize,
    context_gap_priority_address_count: usize,
    invalidation_condition_kind_address_count: usize,
    invalidation_condition_priority_address_count: usize,
    epistemic_action_address_count: usize,
    epistemic_action_reason_address_count: usize,
    answerability_question_address_count: usize,
    evidence_source_address_count: usize,
    max_evidence_confidence_address_count: usize,
    system_time_address_count: usize,
    valid_time_address_count: usize,
    dependency_target_address_count: usize,
    dependency_kind_address_count: usize,
    commit_manifests: Vec<FileKernelPersistentCommitManifestAddress>,
    cells: Vec<FileKernelPersistentCellAddress>,
    semantic_anchors: Vec<FileKernelPersistentSemanticAnchorAddress>,
    commits: Vec<FileKernelPersistentCommitAddress>,
    scopes: Vec<FileKernelPersistentScopeAddress>,
    activations: Vec<FileKernelPersistentActivationAddress>,
    lifecycle_stages: Vec<FileKernelPersistentLifecycleStageAddress>,
    retention_policies: Vec<FileKernelPersistentRetentionPolicyAddress>,
    use_policies: Vec<FileKernelPersistentUsePolicyAddress>,
    promotion_policies: Vec<FileKernelPersistentPromotionPolicyAddress>,
    projection_kinds: Vec<FileKernelPersistentProjectionKindAddress>,
    native_uncertainties: Vec<FileKernelPersistentNativeUncertaintyAddress>,
    native_surprise_bits: Vec<FileKernelPersistentNativeSurpriseBitsAddress>,
    native_saliences: Vec<FileKernelPersistentNativeSalienceAddress>,
    native_context_affordances: Vec<FileKernelPersistentNativeContextAffordanceAddress>,
    native_epistemic_pressures: Vec<FileKernelPersistentNativeEpistemicPressureAddress>,
    trajectory_memory_strategies: Vec<FileKernelPersistentTrajectoryMemoryStrategyAddress>,
    trajectory_memory_confidences: Vec<FileKernelPersistentTrajectoryMemoryConfidenceAddress>,
    context_gap_kinds: Vec<FileKernelPersistentContextGapKindAddress>,
    context_gap_priorities: Vec<FileKernelPersistentContextGapPriorityAddress>,
    invalidation_condition_kinds: Vec<FileKernelPersistentInvalidationConditionKindAddress>,
    invalidation_condition_priorities:
        Vec<FileKernelPersistentInvalidationConditionPriorityAddress>,
    epistemic_actions: Vec<FileKernelPersistentEpistemicActionAddress>,
    epistemic_action_reasons: Vec<FileKernelPersistentEpistemicActionReasonAddress>,
    answerability_questions: Vec<FileKernelPersistentAnswerabilityQuestionAddress>,
    evidence_sources: Vec<FileKernelPersistentEvidenceSourceAddress>,
    max_evidence_confidences: Vec<FileKernelPersistentMaxEvidenceConfidenceAddress>,
    system_times: Vec<FileKernelPersistentSystemTimeAddress>,
    valid_times: Vec<FileKernelPersistentValidTimeAddress>,
    dependency_targets: Vec<FileKernelPersistentDependencyTargetAddress>,
    dependency_kinds: Vec<FileKernelPersistentDependencyKindAddress>,
    revision_link_sources: Vec<FileKernelPersistentRevisionLinkSourceAddress>,
    revision_link_targets: Vec<FileKernelPersistentRevisionLinkTargetAddress>,
    revision_link_kinds: Vec<FileKernelPersistentRevisionLinkKindAddress>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
struct FileKernelPersistentCommitManifestAddress {
    commit_id: CommitId,
    offset: u64,
    length: u64,
    checksum: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
struct FileKernelPersistentCellAddress {
    cell_id: StateCellId,
    offset: u64,
    length: u64,
    checksum: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
struct FileKernelPersistentSemanticAnchorAddress {
    anchor: String,
    cell_id: StateCellId,
    offset: u64,
    length: u64,
    checksum: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
struct FileKernelPersistentCommitAddress {
    commit_id: CommitId,
    cell_id: StateCellId,
    offset: u64,
    length: u64,
    checksum: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
struct FileKernelPersistentScopeAddress {
    scope: Scope,
    cell_id: StateCellId,
    offset: u64,
    length: u64,
    checksum: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
struct FileKernelPersistentActivationAddress {
    activation: ActivationState,
    cell_id: StateCellId,
    offset: u64,
    length: u64,
    checksum: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
struct FileKernelPersistentLifecycleStageAddress {
    lifecycle_stage: LifecycleStage,
    cell_id: StateCellId,
    offset: u64,
    length: u64,
    checksum: String,
}

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
struct FileKernelPersistentRetentionPolicyAddress {
    policy: RetentionPolicy,
    cell_id: StateCellId,
    offset: u64,
    length: u64,
    checksum: String,
}

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
struct FileKernelPersistentUsePolicyAddress {
    policy: UsePolicy,
    cell_id: StateCellId,
    offset: u64,
    length: u64,
    checksum: String,
}

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
struct FileKernelPersistentPromotionPolicyAddress {
    policy: PromotionPolicy,
    cell_id: StateCellId,
    offset: u64,
    length: u64,
    checksum: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
struct FileKernelPersistentProjectionKindAddress {
    projection_kind: MemoryProjectionKind,
    cell_id: StateCellId,
    offset: u64,
    length: u64,
    checksum: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
struct FileKernelPersistentNativeUncertaintyAddress {
    uncertainty_microunits: u32,
    cell_id: StateCellId,
    offset: u64,
    length: u64,
    checksum: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
struct FileKernelPersistentNativeSurpriseBitsAddress {
    surprise_microbits: u64,
    cell_id: StateCellId,
    offset: u64,
    length: u64,
    checksum: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
struct FileKernelPersistentNativeSalienceAddress {
    salience_microunits: u32,
    cell_id: StateCellId,
    offset: u64,
    length: u64,
    checksum: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
struct FileKernelPersistentNativeContextAffordanceAddress {
    context_affordance_microunits: u32,
    cell_id: StateCellId,
    offset: u64,
    length: u64,
    checksum: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
struct FileKernelPersistentNativeEpistemicPressureAddress {
    pressure_microunits: u32,
    cell_id: StateCellId,
    offset: u64,
    length: u64,
    checksum: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
struct FileKernelPersistentTrajectoryMemoryStrategyAddress {
    strategy: ContextPacketStrategy,
    cell_id: StateCellId,
    offset: u64,
    length: u64,
    checksum: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
struct FileKernelPersistentTrajectoryMemoryConfidenceAddress {
    confidence_microunits: u32,
    cell_id: StateCellId,
    offset: u64,
    length: u64,
    checksum: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
struct FileKernelPersistentContextGapKindAddress {
    kind: ContextGapKind,
    cell_id: StateCellId,
    offset: u64,
    length: u64,
    checksum: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
struct FileKernelPersistentContextGapPriorityAddress {
    priority_microunits: u32,
    cell_id: StateCellId,
    offset: u64,
    length: u64,
    checksum: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
struct FileKernelPersistentInvalidationConditionKindAddress {
    kind: InvalidationConditionKind,
    cell_id: StateCellId,
    offset: u64,
    length: u64,
    checksum: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
struct FileKernelPersistentInvalidationConditionPriorityAddress {
    priority_microunits: u32,
    cell_id: StateCellId,
    offset: u64,
    length: u64,
    checksum: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
struct FileKernelPersistentEpistemicActionAddress {
    action: EpistemicAction,
    cell_id: StateCellId,
    offset: u64,
    length: u64,
    checksum: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
struct FileKernelPersistentEpistemicActionReasonAddress {
    reason: EpistemicActionReason,
    cell_id: StateCellId,
    offset: u64,
    length: u64,
    checksum: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
struct FileKernelPersistentAnswerabilityQuestionAddress {
    question: String,
    cell_id: StateCellId,
    offset: u64,
    length: u64,
    checksum: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
struct FileKernelPersistentEvidenceSourceAddress {
    source: String,
    cell_id: StateCellId,
    offset: u64,
    length: u64,
    checksum: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
struct FileKernelPersistentMaxEvidenceConfidenceAddress {
    confidence_microunits: u32,
    cell_id: StateCellId,
    offset: u64,
    length: u64,
    checksum: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
struct FileKernelPersistentSystemTimeAddress {
    system_from: DateTime<Utc>,
    cell_id: StateCellId,
    offset: u64,
    length: u64,
    checksum: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
struct FileKernelPersistentValidTimeAddress {
    valid_from: DateTime<Utc>,
    cell_id: StateCellId,
    offset: u64,
    length: u64,
    checksum: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
struct FileKernelPersistentDependencyTargetAddress {
    target: StateCellId,
    cell_id: StateCellId,
    offset: u64,
    length: u64,
    checksum: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
struct FileKernelPersistentDependencyKindAddress {
    kind: CellDependencyKind,
    cell_id: StateCellId,
    offset: u64,
    length: u64,
    checksum: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
struct FileKernelPersistentRevisionLinkSourceAddress {
    source: StateCellId,
    target: StateCellId,
    kind: RevisionLinkKind,
    offset: u64,
    length: u64,
    checksum: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
struct FileKernelPersistentRevisionLinkTargetAddress {
    target: StateCellId,
    source: StateCellId,
    kind: RevisionLinkKind,
    offset: u64,
    length: u64,
    checksum: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
struct FileKernelPersistentRevisionLinkKindAddress {
    kind: RevisionLinkKind,
    source: StateCellId,
    target: StateCellId,
    offset: u64,
    length: u64,
    checksum: String,
}

trait FileKernelPersistentCellAddressParts {
    fn cell_id(&self) -> StateCellId;
    fn offset(&self) -> u64;
    fn length(&self) -> u64;
    fn checksum(&self) -> &str;
}

trait FileKernelPersistentCommitManifestAddressParts {
    fn commit_id(&self) -> CommitId;
    fn offset(&self) -> u64;
    fn length(&self) -> u64;
    fn checksum(&self) -> &str;
}

trait FileKernelPersistentRevisionLinkAddressParts {
    fn source(&self) -> StateCellId;
    fn target(&self) -> StateCellId;
    fn kind(&self) -> RevisionLinkKind;
    fn offset(&self) -> u64;
    fn length(&self) -> u64;
    fn checksum(&self) -> &str;
}

impl FileKernelPersistentCommitManifestAddressParts for FileKernelPersistentCommitManifestAddress {
    fn commit_id(&self) -> CommitId {
        self.commit_id
    }

    fn offset(&self) -> u64 {
        self.offset
    }

    fn length(&self) -> u64 {
        self.length
    }

    fn checksum(&self) -> &str {
        &self.checksum
    }
}

impl FileKernelPersistentCellAddressParts for FileKernelPersistentCellAddress {
    fn cell_id(&self) -> StateCellId {
        self.cell_id
    }

    fn offset(&self) -> u64 {
        self.offset
    }

    fn length(&self) -> u64 {
        self.length
    }

    fn checksum(&self) -> &str {
        &self.checksum
    }
}

impl FileKernelPersistentCellAddressParts for FileKernelPersistentSemanticAnchorAddress {
    fn cell_id(&self) -> StateCellId {
        self.cell_id
    }

    fn offset(&self) -> u64 {
        self.offset
    }

    fn length(&self) -> u64 {
        self.length
    }

    fn checksum(&self) -> &str {
        &self.checksum
    }
}

impl FileKernelPersistentCellAddressParts for FileKernelPersistentCommitAddress {
    fn cell_id(&self) -> StateCellId {
        self.cell_id
    }

    fn offset(&self) -> u64 {
        self.offset
    }

    fn length(&self) -> u64 {
        self.length
    }

    fn checksum(&self) -> &str {
        &self.checksum
    }
}

impl FileKernelPersistentCellAddressParts for FileKernelPersistentScopeAddress {
    fn cell_id(&self) -> StateCellId {
        self.cell_id
    }

    fn offset(&self) -> u64 {
        self.offset
    }

    fn length(&self) -> u64 {
        self.length
    }

    fn checksum(&self) -> &str {
        &self.checksum
    }
}

impl FileKernelPersistentCellAddressParts for FileKernelPersistentActivationAddress {
    fn cell_id(&self) -> StateCellId {
        self.cell_id
    }

    fn offset(&self) -> u64 {
        self.offset
    }

    fn length(&self) -> u64 {
        self.length
    }

    fn checksum(&self) -> &str {
        &self.checksum
    }
}

impl FileKernelPersistentCellAddressParts for FileKernelPersistentLifecycleStageAddress {
    fn cell_id(&self) -> StateCellId {
        self.cell_id
    }

    fn offset(&self) -> u64 {
        self.offset
    }

    fn length(&self) -> u64 {
        self.length
    }

    fn checksum(&self) -> &str {
        &self.checksum
    }
}

impl FileKernelPersistentCellAddressParts for FileKernelPersistentRetentionPolicyAddress {
    fn cell_id(&self) -> StateCellId {
        self.cell_id
    }

    fn offset(&self) -> u64 {
        self.offset
    }

    fn length(&self) -> u64 {
        self.length
    }

    fn checksum(&self) -> &str {
        &self.checksum
    }
}

impl FileKernelPersistentCellAddressParts for FileKernelPersistentUsePolicyAddress {
    fn cell_id(&self) -> StateCellId {
        self.cell_id
    }

    fn offset(&self) -> u64 {
        self.offset
    }

    fn length(&self) -> u64 {
        self.length
    }

    fn checksum(&self) -> &str {
        &self.checksum
    }
}

impl FileKernelPersistentCellAddressParts for FileKernelPersistentPromotionPolicyAddress {
    fn cell_id(&self) -> StateCellId {
        self.cell_id
    }

    fn offset(&self) -> u64 {
        self.offset
    }

    fn length(&self) -> u64 {
        self.length
    }

    fn checksum(&self) -> &str {
        &self.checksum
    }
}

impl FileKernelPersistentCellAddressParts for FileKernelPersistentProjectionKindAddress {
    fn cell_id(&self) -> StateCellId {
        self.cell_id
    }

    fn offset(&self) -> u64 {
        self.offset
    }

    fn length(&self) -> u64 {
        self.length
    }

    fn checksum(&self) -> &str {
        &self.checksum
    }
}

impl FileKernelPersistentCellAddressParts for FileKernelPersistentNativeUncertaintyAddress {
    fn cell_id(&self) -> StateCellId {
        self.cell_id
    }

    fn offset(&self) -> u64 {
        self.offset
    }

    fn length(&self) -> u64 {
        self.length
    }

    fn checksum(&self) -> &str {
        &self.checksum
    }
}

impl FileKernelPersistentCellAddressParts for FileKernelPersistentNativeSurpriseBitsAddress {
    fn cell_id(&self) -> StateCellId {
        self.cell_id
    }

    fn offset(&self) -> u64 {
        self.offset
    }

    fn length(&self) -> u64 {
        self.length
    }

    fn checksum(&self) -> &str {
        &self.checksum
    }
}

impl FileKernelPersistentCellAddressParts for FileKernelPersistentNativeSalienceAddress {
    fn cell_id(&self) -> StateCellId {
        self.cell_id
    }

    fn offset(&self) -> u64 {
        self.offset
    }

    fn length(&self) -> u64 {
        self.length
    }

    fn checksum(&self) -> &str {
        &self.checksum
    }
}

impl FileKernelPersistentCellAddressParts for FileKernelPersistentNativeEpistemicPressureAddress {
    fn cell_id(&self) -> StateCellId {
        self.cell_id
    }

    fn offset(&self) -> u64 {
        self.offset
    }

    fn length(&self) -> u64 {
        self.length
    }

    fn checksum(&self) -> &str {
        &self.checksum
    }
}

impl FileKernelPersistentCellAddressParts for FileKernelPersistentNativeContextAffordanceAddress {
    fn cell_id(&self) -> StateCellId {
        self.cell_id
    }

    fn offset(&self) -> u64 {
        self.offset
    }

    fn length(&self) -> u64 {
        self.length
    }

    fn checksum(&self) -> &str {
        &self.checksum
    }
}

impl FileKernelPersistentCellAddressParts for FileKernelPersistentTrajectoryMemoryStrategyAddress {
    fn cell_id(&self) -> StateCellId {
        self.cell_id
    }

    fn offset(&self) -> u64 {
        self.offset
    }

    fn length(&self) -> u64 {
        self.length
    }

    fn checksum(&self) -> &str {
        &self.checksum
    }
}

impl FileKernelPersistentCellAddressParts
    for FileKernelPersistentTrajectoryMemoryConfidenceAddress
{
    fn cell_id(&self) -> StateCellId {
        self.cell_id
    }

    fn offset(&self) -> u64 {
        self.offset
    }

    fn length(&self) -> u64 {
        self.length
    }

    fn checksum(&self) -> &str {
        &self.checksum
    }
}

impl FileKernelPersistentCellAddressParts for FileKernelPersistentContextGapKindAddress {
    fn cell_id(&self) -> StateCellId {
        self.cell_id
    }

    fn offset(&self) -> u64 {
        self.offset
    }

    fn length(&self) -> u64 {
        self.length
    }

    fn checksum(&self) -> &str {
        &self.checksum
    }
}

impl FileKernelPersistentCellAddressParts for FileKernelPersistentContextGapPriorityAddress {
    fn cell_id(&self) -> StateCellId {
        self.cell_id
    }

    fn offset(&self) -> u64 {
        self.offset
    }

    fn length(&self) -> u64 {
        self.length
    }

    fn checksum(&self) -> &str {
        &self.checksum
    }
}

impl FileKernelPersistentCellAddressParts for FileKernelPersistentInvalidationConditionKindAddress {
    fn cell_id(&self) -> StateCellId {
        self.cell_id
    }

    fn offset(&self) -> u64 {
        self.offset
    }

    fn length(&self) -> u64 {
        self.length
    }

    fn checksum(&self) -> &str {
        &self.checksum
    }
}

impl FileKernelPersistentCellAddressParts
    for FileKernelPersistentInvalidationConditionPriorityAddress
{
    fn cell_id(&self) -> StateCellId {
        self.cell_id
    }

    fn offset(&self) -> u64 {
        self.offset
    }

    fn length(&self) -> u64 {
        self.length
    }

    fn checksum(&self) -> &str {
        &self.checksum
    }
}

impl FileKernelPersistentCellAddressParts for FileKernelPersistentEpistemicActionAddress {
    fn cell_id(&self) -> StateCellId {
        self.cell_id
    }

    fn offset(&self) -> u64 {
        self.offset
    }

    fn length(&self) -> u64 {
        self.length
    }

    fn checksum(&self) -> &str {
        &self.checksum
    }
}

impl FileKernelPersistentCellAddressParts for FileKernelPersistentEpistemicActionReasonAddress {
    fn cell_id(&self) -> StateCellId {
        self.cell_id
    }

    fn offset(&self) -> u64 {
        self.offset
    }

    fn length(&self) -> u64 {
        self.length
    }

    fn checksum(&self) -> &str {
        &self.checksum
    }
}

impl FileKernelPersistentCellAddressParts for FileKernelPersistentAnswerabilityQuestionAddress {
    fn cell_id(&self) -> StateCellId {
        self.cell_id
    }

    fn offset(&self) -> u64 {
        self.offset
    }

    fn length(&self) -> u64 {
        self.length
    }

    fn checksum(&self) -> &str {
        &self.checksum
    }
}

impl FileKernelPersistentCellAddressParts for FileKernelPersistentEvidenceSourceAddress {
    fn cell_id(&self) -> StateCellId {
        self.cell_id
    }

    fn offset(&self) -> u64 {
        self.offset
    }

    fn length(&self) -> u64 {
        self.length
    }

    fn checksum(&self) -> &str {
        &self.checksum
    }
}

impl FileKernelPersistentCellAddressParts for FileKernelPersistentMaxEvidenceConfidenceAddress {
    fn cell_id(&self) -> StateCellId {
        self.cell_id
    }

    fn offset(&self) -> u64 {
        self.offset
    }

    fn length(&self) -> u64 {
        self.length
    }

    fn checksum(&self) -> &str {
        &self.checksum
    }
}

impl FileKernelPersistentCellAddressParts for FileKernelPersistentSystemTimeAddress {
    fn cell_id(&self) -> StateCellId {
        self.cell_id
    }

    fn offset(&self) -> u64 {
        self.offset
    }

    fn length(&self) -> u64 {
        self.length
    }

    fn checksum(&self) -> &str {
        &self.checksum
    }
}

impl FileKernelPersistentCellAddressParts for FileKernelPersistentValidTimeAddress {
    fn cell_id(&self) -> StateCellId {
        self.cell_id
    }

    fn offset(&self) -> u64 {
        self.offset
    }

    fn length(&self) -> u64 {
        self.length
    }

    fn checksum(&self) -> &str {
        &self.checksum
    }
}

impl FileKernelPersistentCellAddressParts for FileKernelPersistentDependencyTargetAddress {
    fn cell_id(&self) -> StateCellId {
        self.cell_id
    }

    fn offset(&self) -> u64 {
        self.offset
    }

    fn length(&self) -> u64 {
        self.length
    }

    fn checksum(&self) -> &str {
        &self.checksum
    }
}

impl FileKernelPersistentCellAddressParts for FileKernelPersistentDependencyKindAddress {
    fn cell_id(&self) -> StateCellId {
        self.cell_id
    }

    fn offset(&self) -> u64 {
        self.offset
    }

    fn length(&self) -> u64 {
        self.length
    }

    fn checksum(&self) -> &str {
        &self.checksum
    }
}

impl FileKernelPersistentRevisionLinkAddressParts
    for FileKernelPersistentRevisionLinkSourceAddress
{
    fn source(&self) -> StateCellId {
        self.source
    }

    fn target(&self) -> StateCellId {
        self.target
    }

    fn kind(&self) -> RevisionLinkKind {
        self.kind
    }

    fn offset(&self) -> u64 {
        self.offset
    }

    fn length(&self) -> u64 {
        self.length
    }

    fn checksum(&self) -> &str {
        &self.checksum
    }
}

impl FileKernelPersistentRevisionLinkAddressParts
    for FileKernelPersistentRevisionLinkTargetAddress
{
    fn source(&self) -> StateCellId {
        self.source
    }

    fn target(&self) -> StateCellId {
        self.target
    }

    fn kind(&self) -> RevisionLinkKind {
        self.kind
    }

    fn offset(&self) -> u64 {
        self.offset
    }

    fn length(&self) -> u64 {
        self.length
    }

    fn checksum(&self) -> &str {
        &self.checksum
    }
}

impl FileKernelPersistentRevisionLinkAddressParts for FileKernelPersistentRevisionLinkKindAddress {
    fn source(&self) -> StateCellId {
        self.source
    }

    fn target(&self) -> StateCellId {
        self.target
    }

    fn kind(&self) -> RevisionLinkKind {
        self.kind
    }

    fn offset(&self) -> u64 {
        self.offset
    }

    fn length(&self) -> u64 {
        self.length
    }

    fn checksum(&self) -> &str {
        &self.checksum
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
struct FileKernelPersistentAddressIndexes {
    commit_manifests: Vec<FileKernelPersistentCommitManifestAddress>,
    cells: Vec<FileKernelPersistentCellAddress>,
    semantic_anchors: Vec<FileKernelPersistentSemanticAnchorAddress>,
    commits: Vec<FileKernelPersistentCommitAddress>,
    scopes: Vec<FileKernelPersistentScopeAddress>,
    activations: Vec<FileKernelPersistentActivationAddress>,
    lifecycle_stages: Vec<FileKernelPersistentLifecycleStageAddress>,
    retention_policies: Vec<FileKernelPersistentRetentionPolicyAddress>,
    use_policies: Vec<FileKernelPersistentUsePolicyAddress>,
    promotion_policies: Vec<FileKernelPersistentPromotionPolicyAddress>,
    projection_kinds: Vec<FileKernelPersistentProjectionKindAddress>,
    native_uncertainties: Vec<FileKernelPersistentNativeUncertaintyAddress>,
    native_surprise_bits: Vec<FileKernelPersistentNativeSurpriseBitsAddress>,
    native_saliences: Vec<FileKernelPersistentNativeSalienceAddress>,
    native_context_affordances: Vec<FileKernelPersistentNativeContextAffordanceAddress>,
    native_epistemic_pressures: Vec<FileKernelPersistentNativeEpistemicPressureAddress>,
    trajectory_memory_strategies: Vec<FileKernelPersistentTrajectoryMemoryStrategyAddress>,
    trajectory_memory_confidences: Vec<FileKernelPersistentTrajectoryMemoryConfidenceAddress>,
    context_gap_kinds: Vec<FileKernelPersistentContextGapKindAddress>,
    context_gap_priorities: Vec<FileKernelPersistentContextGapPriorityAddress>,
    invalidation_condition_kinds: Vec<FileKernelPersistentInvalidationConditionKindAddress>,
    invalidation_condition_priorities:
        Vec<FileKernelPersistentInvalidationConditionPriorityAddress>,
    epistemic_actions: Vec<FileKernelPersistentEpistemicActionAddress>,
    epistemic_action_reasons: Vec<FileKernelPersistentEpistemicActionReasonAddress>,
    answerability_questions: Vec<FileKernelPersistentAnswerabilityQuestionAddress>,
    evidence_sources: Vec<FileKernelPersistentEvidenceSourceAddress>,
    max_evidence_confidences: Vec<FileKernelPersistentMaxEvidenceConfidenceAddress>,
    system_times: Vec<FileKernelPersistentSystemTimeAddress>,
    valid_times: Vec<FileKernelPersistentValidTimeAddress>,
    dependency_targets: Vec<FileKernelPersistentDependencyTargetAddress>,
    dependency_kinds: Vec<FileKernelPersistentDependencyKindAddress>,
    revision_link_sources: Vec<FileKernelPersistentRevisionLinkSourceAddress>,
    revision_link_targets: Vec<FileKernelPersistentRevisionLinkTargetAddress>,
    revision_link_kinds: Vec<FileKernelPersistentRevisionLinkKindAddress>,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct FileKernelLog {
    cells: Vec<StateCell>,
    explicit_manifests: Vec<CommitManifest>,
    revision_links: Vec<RevisionLinkRecord>,
    has_header: bool,
    health: FileKernelHealth,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct FileKernelIndex {
    cells: Vec<StateCell>,
    ids: HashMap<StateCellId, usize>,
    anchors: HashMap<String, Vec<usize>>,
    scopes: HashMap<Scope, Vec<usize>>,
    answerability_questions: HashMap<String, Vec<usize>>,
    evidence_sources: HashMap<String, Vec<usize>>,
    max_evidence_confidences: Vec<(f32, usize)>,
    valid_times: Vec<(DateTime<Utc>, usize)>,
    system_times: Vec<(DateTime<Utc>, usize)>,
    activations: HashMap<ActivationState, Vec<usize>>,
    lifecycle_stages: HashMap<LifecycleStage, Vec<usize>>,
    retention_policies: HashMap<RetentionPolicy, Vec<usize>>,
    use_policies: HashMap<UsePolicy, Vec<usize>>,
    promotion_policies: Vec<(PromotionPolicy, usize)>,
    projection_kinds: HashMap<MemoryProjectionKind, Vec<usize>>,
    epistemic_actions: HashMap<EpistemicAction, Vec<usize>>,
    epistemic_action_reasons: HashMap<EpistemicActionReason, Vec<usize>>,
    context_gap_kinds: HashMap<ContextGapKind, Vec<usize>>,
    context_gap_priorities: Vec<(f32, usize)>,
    invalidation_condition_kinds: HashMap<InvalidationConditionKind, Vec<usize>>,
    invalidation_condition_priorities: Vec<(f32, usize)>,
    trajectory_memory_strategies: HashMap<ContextPacketStrategy, Vec<usize>>,
    trajectory_memory_confidences: Vec<(f32, usize)>,
    native_uncertainties: Vec<(f32, usize)>,
    native_surprise_bits: Vec<(f32, usize)>,
    native_saliences: Vec<(f32, usize)>,
    native_context_affordances: Vec<(f32, usize)>,
    native_epistemic_pressures: Vec<(f32, usize)>,
    dependency_targets: HashMap<StateCellId, Vec<usize>>,
    dependency_kinds: HashMap<CellDependencyKind, Vec<usize>>,
    dependency_target_kinds: HashMap<(StateCellId, CellDependencyKind), Vec<usize>>,
    commits: HashMap<CommitId, Vec<usize>>,
    manifests: HashMap<CommitId, CommitManifest>,
    manifest_order: Vec<CommitId>,
    revision_links: Vec<RevisionLinkRecord>,
    revision_link_sources: HashMap<StateCellId, Vec<usize>>,
    revision_link_targets: HashMap<StateCellId, Vec<usize>>,
    revision_link_kinds: HashMap<RevisionLinkKind, Vec<usize>>,
    revision_link_source_kinds: HashMap<(StateCellId, RevisionLinkKind), Vec<usize>>,
    revision_link_target_kinds: HashMap<(StateCellId, RevisionLinkKind), Vec<usize>>,
    revision_link_source_targets: HashMap<(StateCellId, StateCellId), Vec<usize>>,
    revision_link_source_target_kinds:
        HashMap<(StateCellId, StateCellId, RevisionLinkKind), Vec<usize>>,
}

struct IndexedCandidateConstraint {
    name: &'static str,
    positions: Vec<usize>,
}

impl FileKernelIndex {
    fn rebuild(log: FileKernelLog) -> Result<Self, KernelError> {
        let mut index = Self::default();
        let has_header = log.has_header;
        for cell in log.cells {
            index.insert(cell)?;
        }
        let mut explicit_commit_ids = HashSet::new();
        for manifest in log.explicit_manifests {
            if !explicit_commit_ids.insert(manifest.commit_id) {
                return Err(KernelError::StoreCorrupt);
            }
            index.apply_explicit_manifest(manifest)?;
        }
        if has_header
            && index
                .commits
                .keys()
                .any(|commit_id| !explicit_commit_ids.contains(commit_id))
        {
            return Err(KernelError::StoreCorrupt);
        }
        for revision_link in log.revision_links {
            index.insert_revision_link(revision_link)?;
        }
        Ok(index)
    }

    fn insert(&mut self, cell: StateCell) -> Result<(), KernelError> {
        if self.ids.contains_key(&cell.id) {
            return Err(KernelError::DuplicateCell);
        }

        let position = self.cells.len();
        self.ids.insert(cell.id, position);
        for anchor in &cell.anchors {
            self.anchors
                .entry(anchor.as_str().to_string())
                .or_default()
                .push(position);
        }
        self.scopes
            .entry(cell.scope.clone())
            .or_default()
            .push(position);
        for question in cell.answerability.questions() {
            self.answerability_questions
                .entry(question.clone())
                .or_default()
                .push(position);
        }
        for evidence in &cell.evidence {
            self.evidence_sources
                .entry(evidence.source.as_str().to_string())
                .or_default()
                .push(position);
        }
        self.max_evidence_confidences
            .push((max_evidence_confidence(&cell), position));
        self.valid_times.push((cell.valid_time.from(), position));
        self.system_times.push((cell.system_time.from(), position));
        self.activations
            .entry(cell.activation)
            .or_default()
            .push(position);
        self.lifecycle_stages
            .entry(cell.lifecycle_stage)
            .or_default()
            .push(position);
        self.retention_policies
            .entry(cell.lifecycle_policy.retention)
            .or_default()
            .push(position);
        self.use_policies
            .entry(cell.lifecycle_policy.use_policy)
            .or_default()
            .push(position);
        self.promotion_policies
            .push((cell.lifecycle_policy.promotion, position));
        for projection in &cell.projections {
            self.projection_kinds
                .entry(projection.kind)
                .or_default()
                .push(position);
        }
        self.epistemic_actions
            .entry(cell.epistemic_action())
            .or_default()
            .push(position);
        for reason in cell.epistemic_action_reasons() {
            self.epistemic_action_reasons
                .entry(reason)
                .or_default()
                .push(position);
        }
        for gap in &cell.context_gaps {
            self.context_gap_kinds
                .entry(gap.kind)
                .or_default()
                .push(position);
            self.context_gap_priorities
                .push((gap.priority.value(), position));
        }
        for condition in &cell.invalidation_conditions {
            self.invalidation_condition_kinds
                .entry(condition.kind)
                .or_default()
                .push(position);
            self.invalidation_condition_priorities
                .push((condition.priority.value(), position));
        }
        if let Some(trajectory_memory) = &cell.trajectory_memory {
            self.trajectory_memory_strategies
                .entry(trajectory_memory.checkout_strategy)
                .or_default()
                .push(position);
            self.trajectory_memory_confidences
                .push((trajectory_memory.confidence.value(), position));
        }
        self.native_uncertainties
            .push((cell.uncertainty.score.value(), position));
        self.native_surprise_bits
            .push((cell.uncertainty.surprise_bits, position));
        self.native_saliences
            .push((cell.attention.salience_score(), position));
        self.native_context_affordances
            .push((cell.context_affordance.context_affordance_score(), position));
        self.native_epistemic_pressures
            .push((cell.epistemic_pressure().checkout_pressure, position));
        for dependency in &cell.dependencies {
            self.dependency_targets
                .entry(dependency.target)
                .or_default()
                .push(position);
            self.dependency_kinds
                .entry(dependency.kind)
                .or_default()
                .push(position);
            self.dependency_target_kinds
                .entry((dependency.target, dependency.kind))
                .or_default()
                .push(position);
        }
        self.commits
            .entry(cell.commit_id)
            .or_default()
            .push(position);
        if !self.manifests.contains_key(&cell.commit_id) {
            self.manifest_order.push(cell.commit_id);
        }
        self.manifests
            .entry(cell.commit_id)
            .and_modify(|manifest| manifest.cell_ids.push(cell.id))
            .or_insert_with(|| {
                CommitManifest::new(cell.commit_id, cell.system_time.from(), vec![cell.id])
            });
        self.cells.push(cell);
        Ok(())
    }

    fn positions_with_minimum_confidence(&self, minimum_confidence: Confidence) -> Vec<usize> {
        self.max_evidence_confidences
            .iter()
            .filter(|(confidence, _position)| *confidence >= minimum_confidence.value())
            .map(|(_confidence, position)| *position)
            .collect()
    }

    fn positions_with_minimum_uncertainty(&self, minimum_uncertainty: Confidence) -> Vec<usize> {
        self.native_uncertainties
            .iter()
            .filter(|(uncertainty, _position)| *uncertainty >= minimum_uncertainty.value())
            .map(|(_uncertainty, position)| *position)
            .collect()
    }

    fn positions_with_minimum_surprise_bits(&self, minimum_surprise_bits: f32) -> Vec<usize> {
        self.native_surprise_bits
            .iter()
            .filter(|(surprise_bits, _position)| *surprise_bits >= minimum_surprise_bits)
            .map(|(_surprise_bits, position)| *position)
            .collect()
    }

    fn positions_with_minimum_salience(&self, minimum_salience: f32) -> Vec<usize> {
        self.native_saliences
            .iter()
            .filter(|(salience, _position)| *salience >= minimum_salience)
            .map(|(_salience, position)| *position)
            .collect()
    }

    fn positions_with_minimum_context_affordance(
        &self,
        minimum_context_affordance: f32,
    ) -> Vec<usize> {
        self.native_context_affordances
            .iter()
            .filter(|(context_affordance, _position)| {
                *context_affordance >= minimum_context_affordance
            })
            .map(|(_context_affordance, position)| *position)
            .collect()
    }

    fn positions_with_minimum_epistemic_pressure(
        &self,
        minimum_epistemic_pressure: f32,
    ) -> Vec<usize> {
        self.native_epistemic_pressures
            .iter()
            .filter(|(pressure, _position)| *pressure >= minimum_epistemic_pressure)
            .map(|(_pressure, position)| *position)
            .collect()
    }

    fn positions_with_minimum_context_gap_priority(
        &self,
        minimum_context_gap_priority: Confidence,
    ) -> Vec<usize> {
        self.context_gap_priorities
            .iter()
            .filter(|(priority, _position)| *priority >= minimum_context_gap_priority.value())
            .map(|(_priority, position)| *position)
            .collect()
    }

    fn positions_with_minimum_invalidation_priority(
        &self,
        minimum_invalidation_priority: Confidence,
    ) -> Vec<usize> {
        self.invalidation_condition_priorities
            .iter()
            .filter(|(priority, _position)| *priority >= minimum_invalidation_priority.value())
            .map(|(_priority, position)| *position)
            .collect()
    }

    fn positions_with_minimum_trajectory_memory_confidence(
        &self,
        minimum_confidence: Confidence,
    ) -> Vec<usize> {
        self.trajectory_memory_confidences
            .iter()
            .filter(|(confidence, _position)| *confidence >= minimum_confidence.value())
            .map(|(_confidence, position)| *position)
            .collect()
    }

    fn positions_at_system_time(&self, system_at: DateTime<Utc>) -> Vec<usize> {
        self.system_times
            .iter()
            .filter(|(system_from, _position)| *system_from <= system_at)
            .map(|(_system_from, position)| *position)
            .collect()
    }

    fn positions_at_valid_time(&self, valid_at: DateTime<Utc>) -> Vec<usize> {
        self.valid_times
            .iter()
            .filter(|(valid_from, _position)| *valid_from <= valid_at)
            .map(|(_valid_from, position)| *position)
            .collect()
    }

    fn indexed_candidate_constraints(
        &self,
        lookup: &CellLookup,
    ) -> Vec<IndexedCandidateConstraint> {
        let mut candidates = Vec::new();

        if let Some(cell_id) = lookup.cell_id {
            candidates.push(IndexedCandidateConstraint {
                name: "cell_id",
                positions: self
                    .position_by_id(cell_id)
                    .map(|position| vec![position])
                    .unwrap_or_default(),
            });
        }
        if let Some(anchor) = lookup.semantic_anchor.as_ref() {
            candidates.push(IndexedCandidateConstraint {
                name: "semantic_anchor",
                positions: self.anchors.get(anchor).cloned().unwrap_or_default(),
            });
        }
        if let Some(commit_id) = lookup.commit_id {
            candidates.push(IndexedCandidateConstraint {
                name: "commit_id",
                positions: self.commits.get(&commit_id).cloned().unwrap_or_default(),
            });
        }
        if let Some(scope) = lookup.scope.as_ref() {
            candidates.push(IndexedCandidateConstraint {
                name: "scope",
                positions: self.scopes.get(scope).cloned().unwrap_or_default(),
            });
        }
        if let Some(question) = lookup.answerability_question.as_ref() {
            candidates.push(IndexedCandidateConstraint {
                name: "answerability_question",
                positions: self
                    .answerability_questions
                    .get(question)
                    .cloned()
                    .unwrap_or_default(),
            });
        }
        if let Some(source) = lookup.evidence_source.as_ref() {
            candidates.push(IndexedCandidateConstraint {
                name: "evidence_source",
                positions: self
                    .evidence_sources
                    .get(source)
                    .cloned()
                    .unwrap_or_default(),
            });
        }
        if let Some(activation) = lookup.activation {
            candidates.push(IndexedCandidateConstraint {
                name: "activation",
                positions: self
                    .activations
                    .get(&activation)
                    .cloned()
                    .unwrap_or_default(),
            });
        }
        if let Some(lifecycle_stage) = lookup.lifecycle_stage {
            candidates.push(IndexedCandidateConstraint {
                name: "lifecycle_stage",
                positions: self
                    .lifecycle_stages
                    .get(&lifecycle_stage)
                    .cloned()
                    .unwrap_or_default(),
            });
        }
        if let Some(policy) = lookup.retention_policy {
            candidates.push(IndexedCandidateConstraint {
                name: "retention_policy",
                positions: self
                    .retention_policies
                    .get(&policy)
                    .cloned()
                    .unwrap_or_default(),
            });
        }
        if let Some(policy) = lookup.use_policy {
            candidates.push(IndexedCandidateConstraint {
                name: "use_policy",
                positions: self.use_policies.get(&policy).cloned().unwrap_or_default(),
            });
        }
        if let Some(policy) = lookup.promotion_policy {
            candidates.push(IndexedCandidateConstraint {
                name: "promotion_policy",
                positions: self
                    .promotion_policies
                    .iter()
                    .filter_map(|(candidate, position)| {
                        if *candidate == policy {
                            Some(*position)
                        } else {
                            None
                        }
                    })
                    .collect(),
            });
        }
        if let Some(projection_kind) = lookup.projection_kind {
            candidates.push(IndexedCandidateConstraint {
                name: "projection_kind",
                positions: self
                    .projection_kinds
                    .get(&projection_kind)
                    .cloned()
                    .unwrap_or_default(),
            });
        }
        if let Some(action) = lookup.epistemic_action {
            candidates.push(IndexedCandidateConstraint {
                name: "epistemic_action",
                positions: self
                    .epistemic_actions
                    .get(&action)
                    .cloned()
                    .unwrap_or_default(),
            });
        }
        if let Some(reason) = lookup.epistemic_action_reason {
            candidates.push(IndexedCandidateConstraint {
                name: "epistemic_action_reason",
                positions: self
                    .epistemic_action_reasons
                    .get(&reason)
                    .cloned()
                    .unwrap_or_default(),
            });
        }
        if let Some(kind) = lookup.context_gap_kind {
            candidates.push(IndexedCandidateConstraint {
                name: "context_gap_kind",
                positions: self
                    .context_gap_kinds
                    .get(&kind)
                    .cloned()
                    .unwrap_or_default(),
            });
        }
        if let Some(minimum_priority) = lookup.minimum_context_gap_priority {
            candidates.push(IndexedCandidateConstraint {
                name: "minimum_context_gap_priority",
                positions: self.positions_with_minimum_context_gap_priority(minimum_priority),
            });
        }
        if let Some(kind) = lookup.invalidation_condition_kind {
            candidates.push(IndexedCandidateConstraint {
                name: "invalidation_condition_kind",
                positions: self
                    .invalidation_condition_kinds
                    .get(&kind)
                    .cloned()
                    .unwrap_or_default(),
            });
        }
        if let Some(minimum_priority) = lookup.minimum_invalidation_priority {
            candidates.push(IndexedCandidateConstraint {
                name: "minimum_invalidation_priority",
                positions: self.positions_with_minimum_invalidation_priority(minimum_priority),
            });
        }
        if let Some(minimum_uncertainty) = lookup.minimum_uncertainty {
            candidates.push(IndexedCandidateConstraint {
                name: "minimum_uncertainty",
                positions: self.positions_with_minimum_uncertainty(minimum_uncertainty),
            });
        }
        if let Some(minimum_surprise_bits) = lookup.minimum_surprise_bits {
            candidates.push(IndexedCandidateConstraint {
                name: "minimum_surprise_bits",
                positions: self.positions_with_minimum_surprise_bits(minimum_surprise_bits),
            });
        }
        if let Some(minimum_salience) = lookup.minimum_salience {
            candidates.push(IndexedCandidateConstraint {
                name: "minimum_salience",
                positions: self.positions_with_minimum_salience(minimum_salience),
            });
        }
        if let Some(minimum_context_affordance) = lookup.minimum_context_affordance {
            candidates.push(IndexedCandidateConstraint {
                name: "minimum_context_affordance",
                positions: self
                    .positions_with_minimum_context_affordance(minimum_context_affordance),
            });
        }
        if let Some(minimum_epistemic_pressure) = lookup.minimum_epistemic_pressure {
            candidates.push(IndexedCandidateConstraint {
                name: "minimum_epistemic_pressure",
                positions: self
                    .positions_with_minimum_epistemic_pressure(minimum_epistemic_pressure),
            });
        }
        if let Some(strategy) = lookup.trajectory_memory_strategy {
            candidates.push(IndexedCandidateConstraint {
                name: "trajectory_memory_strategy",
                positions: self
                    .trajectory_memory_strategies
                    .get(&strategy)
                    .cloned()
                    .unwrap_or_default(),
            });
        }
        if let Some(minimum_confidence) = lookup.minimum_trajectory_memory_confidence {
            candidates.push(IndexedCandidateConstraint {
                name: "minimum_trajectory_memory_confidence",
                positions: self
                    .positions_with_minimum_trajectory_memory_confidence(minimum_confidence),
            });
        }
        if let Some(target) = lookup.dependency_target {
            let (name, positions) = lookup.dependency_kind.map_or_else(
                || {
                    (
                        "dependency_target",
                        self.dependency_targets
                            .get(&target)
                            .cloned()
                            .unwrap_or_default(),
                    )
                },
                |kind| {
                    (
                        "dependency_target_kind",
                        self.dependency_target_kinds
                            .get(&(target, kind))
                            .cloned()
                            .unwrap_or_default(),
                    )
                },
            );
            candidates.push(IndexedCandidateConstraint { name, positions });
        } else if let Some(kind) = lookup.dependency_kind {
            candidates.push(IndexedCandidateConstraint {
                name: "dependency_kind",
                positions: self
                    .dependency_kinds
                    .get(&kind)
                    .cloned()
                    .unwrap_or_default(),
            });
        }
        if let Some(minimum_confidence) = lookup.minimum_confidence {
            candidates.push(IndexedCandidateConstraint {
                name: "minimum_confidence",
                positions: self.positions_with_minimum_confidence(minimum_confidence),
            });
        }
        if let Some(system_at) = lookup.system_at {
            candidates.push(IndexedCandidateConstraint {
                name: "system_at",
                positions: self.positions_at_system_time(system_at),
            });
        }
        if let Some(valid_at) = lookup.valid_at {
            candidates.push(IndexedCandidateConstraint {
                name: "valid_at",
                positions: self.positions_at_valid_time(valid_at),
            });
        }

        candidates
    }

    fn exact_constraint_names(lookup: &CellLookup) -> Vec<&'static str> {
        let mut constraints = Vec::new();
        if lookup.cell_id.is_some() {
            constraints.push("cell_id");
        }
        if lookup.semantic_anchor.is_some() {
            constraints.push("semantic_anchor");
        }
        if lookup.commit_id.is_some() {
            constraints.push("commit_id");
        }
        if lookup.scope.is_some() {
            constraints.push("scope");
        }
        if lookup.answerability_question.is_some() {
            constraints.push("answerability_question");
        }
        if lookup.evidence_source.is_some() {
            constraints.push("evidence_source");
        }
        if lookup.activation.is_some() {
            constraints.push("activation");
        }
        if lookup.lifecycle_stage.is_some() {
            constraints.push("lifecycle_stage");
        }
        if lookup.retention_policy.is_some() {
            constraints.push("retention_policy");
        }
        if lookup.use_policy.is_some() {
            constraints.push("use_policy");
        }
        if lookup.promotion_policy.is_some() {
            constraints.push("promotion_policy");
        }
        if lookup.projection_kind.is_some() {
            constraints.push("projection_kind");
        }
        if lookup.epistemic_action.is_some() {
            constraints.push("epistemic_action");
        }
        if lookup.epistemic_action_reason.is_some() {
            constraints.push("epistemic_action_reason");
        }
        if lookup.selection_reason.is_some() {
            constraints.push("selection_reason");
        }
        if lookup.trajectory_memory_strategy.is_some() {
            constraints.push("trajectory_memory_strategy");
        }
        if lookup.minimum_trajectory_memory_confidence.is_some() {
            constraints.push("minimum_trajectory_memory_confidence");
        }
        if lookup.context_gap_kind.is_some() {
            constraints.push("context_gap_kind");
        }
        if lookup.minimum_context_gap_priority.is_some() {
            constraints.push("minimum_context_gap_priority");
        }
        if lookup.invalidation_condition_kind.is_some() {
            constraints.push("invalidation_condition_kind");
        }
        if lookup.minimum_invalidation_priority.is_some() {
            constraints.push("minimum_invalidation_priority");
        }
        if lookup.minimum_uncertainty.is_some() {
            constraints.push("minimum_uncertainty");
        }
        if lookup.minimum_surprise_bits.is_some() {
            constraints.push("minimum_surprise_bits");
        }
        if lookup.minimum_probability_delta.is_some() {
            constraints.push("minimum_probability_delta");
        }
        if lookup.minimum_salience.is_some() {
            constraints.push("minimum_salience");
        }
        if lookup.minimum_context_affordance.is_some() {
            constraints.push("minimum_context_affordance");
        }
        if lookup.minimum_epistemic_pressure.is_some() {
            constraints.push("minimum_epistemic_pressure");
        }
        match (lookup.dependency_target, lookup.dependency_kind) {
            (Some(_), Some(_)) => constraints.push("dependency_target_kind"),
            (Some(_), None) => constraints.push("dependency_target"),
            (None, Some(_)) => constraints.push("dependency_kind"),
            (None, None) => {}
        }
        if lookup.minimum_confidence.is_some() {
            constraints.push("minimum_confidence");
        }
        if lookup.system_at.is_some() {
            constraints.push("system_at");
        }
        if lookup.valid_at.is_some() {
            constraints.push("valid_at");
        }
        constraints
    }

    fn lossy_indexed_constraint_names(lookup: &CellLookup) -> Vec<&'static str> {
        let mut constraints = Vec::new();
        if lookup.system_at.is_some() {
            constraints.push("system_at");
        }
        if lookup.valid_at.is_some() {
            constraints.push("valid_at");
        }
        constraints
    }

    fn candidate_positions(&self, lookup: &CellLookup) -> Vec<usize> {
        let mut candidates = self
            .indexed_candidate_constraints(lookup)
            .into_iter()
            .map(|constraint| constraint.positions)
            .collect::<Vec<_>>();
        let Some((smallest_index, _positions)) = candidates
            .iter()
            .enumerate()
            .min_by_key(|(_index, positions)| positions.len())
        else {
            return (0..self.cells.len()).collect();
        };
        let mut selected = candidates.swap_remove(smallest_index);
        let remaining_candidates = candidates
            .into_iter()
            .map(|positions| positions.into_iter().collect::<HashSet<_>>())
            .collect::<Vec<_>>();
        selected.retain(|position| {
            remaining_candidates
                .iter()
                .all(|positions| positions.contains(position))
        });
        selected
    }

    fn cell_matches_lookup(cell: &StateCell, lookup: &CellLookup) -> bool {
        lookup.cell_id.map_or(true, |cell_id| cell.id == cell_id)
            && lookup.semantic_anchor.as_ref().map_or(true, |anchor| {
                cell.anchors
                    .iter()
                    .any(|candidate| candidate.as_str() == anchor)
            })
            && lookup
                .scope
                .as_ref()
                .map_or(true, |scope| &cell.scope == scope)
            && lookup
                .valid_at
                .map_or(true, |valid_at| cell.valid_time.contains(valid_at))
            && lookup
                .system_at
                .map_or(true, |system_at| cell.system_time.contains(system_at))
            && lookup
                .commit_id
                .map_or(true, |commit_id| cell.commit_id == commit_id)
            && lookup
                .activation
                .map_or(true, |activation| cell.activation == activation)
            && lookup
                .lifecycle_stage
                .map_or(true, |stage| cell.lifecycle_stage == stage)
            && lookup
                .retention_policy
                .map_or(true, |policy| cell.lifecycle_policy.retention == policy)
            && lookup
                .use_policy
                .map_or(true, |policy| cell.lifecycle_policy.use_policy == policy)
            && lookup
                .promotion_policy
                .map_or(true, |policy| cell.lifecycle_policy.promotion == policy)
            && lookup.projection_kind.map_or(true, |kind| {
                cell.projections
                    .iter()
                    .any(|projection| projection.kind == kind)
            })
            && lookup.minimum_uncertainty.map_or(true, |minimum| {
                cell.uncertainty.score.value() >= minimum.value()
            })
            && lookup
                .minimum_surprise_bits
                .map_or(true, |minimum| cell.uncertainty.surprise_bits >= minimum)
            && lookup.minimum_probability_delta.map_or(true, |minimum| {
                cell.uncertainty
                    .expectation
                    .as_ref()
                    .is_some_and(|expectation| expectation.probability_delta >= minimum)
            })
            && lookup
                .minimum_salience
                .map_or(true, |minimum| cell.attention.salience_score() >= minimum)
            && lookup.minimum_context_affordance.map_or(true, |minimum| {
                cell.context_affordance.context_affordance_score() >= minimum
            })
            && lookup.minimum_epistemic_pressure.map_or(true, |minimum| {
                cell.epistemic_pressure().checkout_pressure >= minimum
            })
            && lookup.context_gap_kind.map_or(true, |kind| {
                cell.context_gaps.iter().any(|gap| gap.kind == kind)
            })
            && lookup.minimum_context_gap_priority.map_or(true, |minimum| {
                cell.context_gaps
                    .iter()
                    .any(|gap| gap.priority.value() >= minimum.value())
            })
            && lookup.invalidation_condition_kind.map_or(true, |kind| {
                cell.invalidation_conditions
                    .iter()
                    .any(|condition| condition.kind == kind)
            })
            && lookup
                .minimum_invalidation_priority
                .map_or(true, |minimum| {
                    cell.invalidation_conditions
                        .iter()
                        .any(|condition| condition.priority.value() >= minimum.value())
                })
            && lookup
                .epistemic_action
                .map_or(true, |action| cell.epistemic_action() == action)
            && lookup.epistemic_action_reason.map_or(true, |reason| {
                cell.epistemic_action_reasons().contains(&reason)
            })
            && lookup.selection_reason.map_or(true, |reason| {
                Self::cell_matches_selection_reason(cell, reason)
            })
            && lookup.trajectory_memory_strategy.map_or(true, |strategy| {
                cell.trajectory_memory
                    .as_ref()
                    .is_some_and(|memory| memory.checkout_strategy == strategy)
            })
            && lookup
                .minimum_trajectory_memory_confidence
                .map_or(true, |minimum| {
                    cell.trajectory_memory
                        .as_ref()
                        .is_some_and(|memory| memory.confidence.value() >= minimum.value())
                })
            && lookup
                .answerability_question
                .as_ref()
                .map_or(true, |question| {
                    cell.answerability
                        .questions()
                        .iter()
                        .any(|candidate| candidate == question)
                })
            && lookup.evidence_source.as_ref().map_or(true, |source| {
                cell.evidence
                    .iter()
                    .any(|evidence| evidence.source.as_str() == source)
            })
            && lookup
                .minimum_confidence
                .map_or(true, |minimum_confidence| {
                    cell.evidence
                        .iter()
                        .any(|evidence| evidence.confidence.value() >= minimum_confidence.value())
                })
            && Self::cell_matches_dependency_lookup(cell, lookup)
    }

    fn cell_matches_selection_reason(
        cell: &StateCell,
        reason: ContextPacketSelectionReason,
    ) -> bool {
        match reason {
            ContextPacketSelectionReason::EvidenceConfidence => cell
                .evidence
                .iter()
                .any(|evidence| evidence.confidence.value() > 0.0),
            ContextPacketSelectionReason::UtilityFeedback => {
                cell.utility_feedback.utility_score()
                    != continuitydb_core::UtilityFeedback::default().utility_score()
            }
            ContextPacketSelectionReason::LifecycleStage => {
                cell.lifecycle_stage != Default::default()
            }
            ContextPacketSelectionReason::ProjectionProfile => !cell.projections.is_empty(),
            ContextPacketSelectionReason::NativeUncertainty => cell.uncertainty.is_recorded(),
            ContextPacketSelectionReason::EpistemicCalibration => cell.calibration.is_recorded(),
            ContextPacketSelectionReason::ContextAffordance => {
                cell.context_affordance.is_recorded()
            }
            ContextPacketSelectionReason::ContextGap => !cell.context_gaps.is_empty(),
            ContextPacketSelectionReason::InvalidationCondition => {
                !cell.invalidation_conditions.is_empty()
            }
            ContextPacketSelectionReason::TrajectoryMemory => cell.trajectory_memory.is_some(),
            ContextPacketSelectionReason::LifecyclePolicy => {
                cell.lifecycle_policy != Default::default()
            }
            ContextPacketSelectionReason::AttentionSignal => cell.attention.is_recorded(),
            ContextPacketSelectionReason::Answerability => {
                !cell.answerability.questions().is_empty()
            }
        }
    }

    fn cell_matches_dependency_lookup(cell: &StateCell, lookup: &CellLookup) -> bool {
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
    }

    fn lookup_plan(&self, lookup: &CellLookup) -> FileKernelLookupPlan {
        let indexed_candidate_constraints = self.indexed_candidate_constraints(lookup);
        let exact_constraints = Self::exact_constraint_names(lookup);
        let lossy_indexed_constraints = Self::lossy_indexed_constraint_names(lookup);
        let indexed_constraints = indexed_candidate_constraints
            .iter()
            .map(|constraint| constraint.name)
            .collect::<Vec<_>>();
        let residual_exact_constraints = exact_constraints
            .iter()
            .copied()
            .filter(|constraint| {
                !indexed_constraints.contains(constraint)
                    || lossy_indexed_constraints.contains(constraint)
            })
            .collect::<Vec<_>>();
        let indexed_constraint_plans = indexed_candidate_constraints
            .iter()
            .map(|constraint| FileKernelIndexedConstraintPlan {
                name: constraint.name,
                candidate_count: constraint.positions.len(),
            })
            .collect::<Vec<_>>();
        let indexed_constraint_count = indexed_candidate_constraints.len();
        let candidate_positions = self.candidate_positions(lookup);
        let candidate_count = candidate_positions.len();
        let exact_match_count = candidate_positions
            .iter()
            .filter(|position| Self::cell_matches_lookup(&self.cells[**position], lookup))
            .count();
        let filtered_candidate_count = candidate_count - exact_match_count;
        let candidate_selectivity_basis_points =
            candidate_selectivity_basis_points(candidate_count, exact_match_count);
        FileKernelLookupPlan {
            indexed_constraint_count,
            indexed_constraints,
            indexed_constraint_plans,
            exact_constraint_count: exact_constraints.len(),
            exact_constraints,
            residual_exact_constraint_count: residual_exact_constraints.len(),
            residual_exact_constraints,
            lossy_indexed_constraint_count: lossy_indexed_constraints.len(),
            lossy_indexed_constraints,
            candidate_count,
            exact_match_count,
            filtered_candidate_count,
            candidate_selectivity_basis_points,
            full_scan: indexed_constraint_count == 0,
        }
    }

    fn apply_explicit_manifest(&mut self, manifest: CommitManifest) -> Result<(), KernelError> {
        let existing_positions = self
            .commits
            .get(&manifest.commit_id)
            .ok_or(KernelError::StoreCorrupt)?;
        let existing_ids = existing_positions
            .iter()
            .map(|position| self.cells[*position].id)
            .collect::<Vec<_>>();
        if existing_ids != manifest.cell_ids {
            return Err(KernelError::StoreCorrupt);
        }

        for cell_id in &manifest.cell_ids {
            let position = self
                .position_by_id(*cell_id)
                .ok_or(KernelError::StoreCorrupt)?;
            let cell = &self.cells[position];
            if cell.commit_id != manifest.commit_id
                || cell.system_time.from() != manifest.committed_at
            {
                return Err(KernelError::StoreCorrupt);
            }
        }

        let manifest_index = self
            .manifest_order
            .iter()
            .position(|commit_id| *commit_id == manifest.commit_id)
            .ok_or(KernelError::StoreCorrupt)?;
        self.manifest_order.remove(manifest_index);
        self.manifest_order.push(manifest.commit_id);
        self.manifests.insert(manifest.commit_id, manifest);
        Ok(())
    }

    fn insert_revision_link(
        &mut self,
        revision_link: RevisionLinkRecord,
    ) -> Result<(), KernelError> {
        if self.revision_links.contains(&revision_link) {
            return Err(KernelError::DuplicateRevisionLink);
        }

        let position = self.revision_links.len();
        self.revision_link_sources
            .entry(revision_link.source)
            .or_default()
            .push(position);
        self.revision_link_targets
            .entry(revision_link.target)
            .or_default()
            .push(position);
        self.revision_link_kinds
            .entry(revision_link.kind)
            .or_default()
            .push(position);
        self.revision_link_source_kinds
            .entry((revision_link.source, revision_link.kind))
            .or_default()
            .push(position);
        self.revision_link_target_kinds
            .entry((revision_link.target, revision_link.kind))
            .or_default()
            .push(position);
        self.revision_link_source_targets
            .entry((revision_link.source, revision_link.target))
            .or_default()
            .push(position);
        self.revision_link_source_target_kinds
            .entry((
                revision_link.source,
                revision_link.target,
                revision_link.kind,
            ))
            .or_default()
            .push(position);
        self.revision_links.push(revision_link);
        Ok(())
    }

    fn contains_id(&self, id: StateCellId) -> bool {
        self.ids.contains_key(&id)
    }

    fn position_by_id(&self, id: StateCellId) -> Option<usize> {
        self.ids.get(&id).copied()
    }

    fn contains_commit(&self, commit_id: CommitId) -> bool {
        self.manifests.contains_key(&commit_id)
    }

    fn list_manifests(&self) -> Vec<CommitManifest> {
        self.manifest_order
            .iter()
            .filter_map(|commit_id| self.manifests.get(commit_id).cloned())
            .collect()
    }

    fn list_manifests_matching(
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
}

/// Append-only JSONL file-backed storage kernel.
#[derive(Clone, Debug)]
pub struct FileKernel {
    path: PathBuf,
    index: FileKernelIndex,
    health: FileKernelHealth,
}

/// Observable status for a file-backed storage kernel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FileKernelStatus {
    /// Number of visible StateCells.
    pub cell_count: usize,
    /// Number of visible commit manifests.
    pub commit_count: usize,
    /// Number of visible revision-link records.
    pub revision_link_count: usize,
    /// Current durable file size in bytes.
    pub file_size_bytes: u64,
}

/// Deterministic summary of how the file kernel will seed a cell lookup.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileKernelLookupPlan {
    /// Number of indexed lookup constraints present in the request.
    pub indexed_constraint_count: usize,
    /// Ordered names of indexed lookup constraints present in the request.
    pub indexed_constraints: Vec<&'static str>,
    /// Ordered per-constraint indexed candidate details.
    pub indexed_constraint_plans: Vec<FileKernelIndexedConstraintPlan>,
    /// Number of exact lookup constraints present in the request.
    pub exact_constraint_count: usize,
    /// Ordered names of exact lookup constraints checked after candidate selection.
    pub exact_constraints: Vec<&'static str>,
    /// Number of exact lookup constraints that require residual filtering after index lookup.
    pub residual_exact_constraint_count: usize,
    /// Ordered exact lookup constraints enforced by residual filtering after index lookup.
    pub residual_exact_constraints: Vec<&'static str>,
    /// Number of indexed lookup constraints that can over-select candidates.
    pub lossy_indexed_constraint_count: usize,
    /// Ordered names of indexed lookup constraints that require exact residual filtering.
    pub lossy_indexed_constraints: Vec<&'static str>,
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

/// Candidate-set contribution for one indexed file-kernel lookup constraint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FileKernelIndexedConstraintPlan {
    /// Stable indexed lookup constraint name.
    pub name: &'static str,
    /// Number of StateCell positions selected by this single index before intersection.
    pub candidate_count: usize,
}

fn candidate_selectivity_basis_points(candidate_count: usize, exact_match_count: usize) -> usize {
    if candidate_count == 0 {
        return 0;
    }
    exact_match_count * 10_000 / candidate_count
}

/// Operational health report for a file-backed storage kernel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FileKernelHealth {
    /// Whether the log starts with the supported current header.
    pub has_header: bool,
    /// Number of legacy raw StateCell records accepted during open.
    pub legacy_raw_cells: usize,
    /// Number of typed cell or commit records that lacked checksums.
    pub checksum_free_records: usize,
    /// Number of typed cell or commit records with valid checksums.
    pub canonical_records: usize,
    /// Whether compaction should rewrite the store into the canonical format.
    pub compaction_recommended: bool,
    /// Whether a persistent index checkpoint sidecar existed when the store was opened.
    pub persistent_index_checkpoint_present_on_open: bool,
    /// Whether the persistent index checkpoint sidecar was valid and trusted when opened.
    pub persistent_index_checkpoint_trusted_on_open: bool,
    /// Whether opening the store had to create or rebuild the persistent index checkpoint.
    pub persistent_index_checkpoint_rebuilt_on_open: bool,
}

impl FileKernelHealth {
    fn from_counts(
        has_header: bool,
        legacy_raw_cells: usize,
        checksum_free_records: usize,
        canonical_records: usize,
    ) -> Self {
        Self {
            has_header,
            legacy_raw_cells,
            checksum_free_records,
            canonical_records,
            compaction_recommended: !has_header
                || legacy_raw_cells > 0
                || checksum_free_records > 0,
            persistent_index_checkpoint_present_on_open: false,
            persistent_index_checkpoint_trusted_on_open: false,
            persistent_index_checkpoint_rebuilt_on_open: false,
        }
    }

    fn with_persistent_index_checkpoint_open_state(
        mut self,
        state: FileKernelPersistentIndexCheckpointOpenState,
    ) -> Self {
        self.persistent_index_checkpoint_present_on_open = state.present_on_open;
        self.persistent_index_checkpoint_trusted_on_open = state.trusted_on_open;
        self.persistent_index_checkpoint_rebuilt_on_open = state.rebuilt_on_open;
        self
    }
}

impl Default for FileKernelHealth {
    fn default() -> Self {
        Self::from_counts(false, 0, 0, 0)
    }
}

impl PartialEq for FileKernel {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path
    }
}

impl Eq for FileKernel {}

impl FileKernel {
    /// Opens an append-only JSONL kernel at the supplied path.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, KernelError> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).map_err(|_error| KernelError::StoreIo)?;
        }

        OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|_error| KernelError::StoreIo)?;

        ensure_file_header(&path)?;
        let log = read_log_from_path(&path)?;
        let checkpoint_state = ensure_persistent_index_checkpoint_for_open(&path)?;
        let health = log
            .health
            .with_persistent_index_checkpoint_open_state(checkpoint_state);
        let index = FileKernelIndex::rebuild(log)?;

        Ok(Self {
            path,
            index,
            health,
        })
    }

    /// Returns the backing file path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns observable status for the backing file store.
    pub fn status(&self) -> Result<FileKernelStatus, KernelError> {
        let file_size_bytes = fs::metadata(&self.path)
            .map_err(|_error| KernelError::StoreIo)?
            .len();
        let checkpoint = persistent_index_checkpoint_or_rebuild(&self.path)?;
        let cell_count = if checkpoint.cells.is_empty() {
            self.index.cells.len()
        } else {
            checkpoint.cells.len()
        };
        let commit_count = if checkpoint.commit_manifests.is_empty() {
            self.index.manifest_order.len()
        } else {
            checkpoint.commit_manifests.len()
        };
        let revision_link_count = if checkpoint.revision_link_sources.is_empty() {
            self.index.revision_links.len()
        } else {
            checkpoint.revision_link_sources.len()
        };
        Ok(FileKernelStatus {
            cell_count,
            commit_count,
            revision_link_count,
            file_size_bytes,
        })
    }

    /// Returns the file-format health report captured for this store.
    pub fn health(&self) -> FileKernelHealth {
        self.health
    }

    /// Returns a deterministic lookup candidate plan for the supplied constraints.
    pub fn lookup_plan(&self, lookup: &CellLookup) -> FileKernelLookupPlan {
        if let Ok(checkpoint) = persistent_index_checkpoint_or_rebuild(&self.path) {
            if !checkpoint.cells.is_empty() {
                if let Ok(index) = file_kernel_index_from_persistent_cell_addresses(
                    &self.path,
                    checkpoint.cells.iter(),
                ) {
                    return index.lookup_plan(lookup);
                }
            }
        }

        self.index.lookup_plan(lookup)
    }

    /// Rewrites the backing JSONL log into the current canonical record format.
    pub fn compact(&mut self) -> Result<(), KernelError> {
        let checkpoint_open_state = FileKernelPersistentIndexCheckpointOpenState {
            present_on_open: self.health.persistent_index_checkpoint_present_on_open,
            trusted_on_open: self.health.persistent_index_checkpoint_trusted_on_open,
            rebuilt_on_open: self.health.persistent_index_checkpoint_rebuilt_on_open,
        };
        let checkpoint = persistent_index_checkpoint_or_rebuild(&self.path)?;
        let cells = if checkpoint.cells.is_empty() {
            self.index.cells.clone()
        } else {
            checkpoint
                .cells
                .iter()
                .map(|address| read_cell_at_persistent_address_parts(&self.path, address))
                .collect::<Result<Vec<_>, _>>()?
        };
        let manifests = if checkpoint.commit_manifests.is_empty() {
            self.index.list_manifests()
        } else {
            checkpoint
                .commit_manifests
                .iter()
                .map(|address| {
                    read_commit_manifest_at_persistent_address_parts(&self.path, address)
                })
                .collect::<Result<Vec<_>, _>>()?
        };
        let revision_links = if checkpoint.revision_link_sources.is_empty() {
            self.index.revision_links.clone()
        } else {
            checkpoint
                .revision_link_sources
                .iter()
                .map(|address| read_revision_link_at_persistent_address_parts(&self.path, address))
                .collect::<Result<Vec<_>, _>>()?
        };
        let encoded = encode_canonical_log(cells.iter(), manifests.iter(), revision_links.iter())?;
        let temp_path = compact_temp_path(&self.path);
        {
            let mut temp_file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&temp_path)
                .map_err(|_error| KernelError::StoreIo)?;
            write_all_durable(&mut temp_file, encoded.as_bytes())?;
        }

        let compacted_log = read_log_from_path(&temp_path)?;
        let compacted_health = compacted_log.health;
        let compacted_index = FileKernelIndex::rebuild(compacted_log)?;
        fs::rename(&temp_path, &self.path).map_err(|_error| KernelError::StoreIo)?;
        sync_parent_directory(&self.path)?;
        self.index = compacted_index;
        self.health =
            compacted_health.with_persistent_index_checkpoint_open_state(checkpoint_open_state);
        refresh_persistent_index_checkpoint(&self.path)?;
        Ok(())
    }
}

fn compact_temp_path(path: &Path) -> PathBuf {
    let suffix = format!("compact-{}", Utc::now().timestamp_micros());
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "continuitydb.jsonl".to_string());
    path.with_file_name(format!("{file_name}.{suffix}"))
}

fn persistent_index_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "continuitydb.jsonl".to_string());
    path.with_file_name(format!("{file_name}.index.json"))
}

fn persistent_index_temp_path(path: &Path) -> PathBuf {
    let suffix = format!("tmp-{}", Utc::now().timestamp_micros());
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "continuitydb.index.json".to_string());
    path.with_file_name(format!("{file_name}.{suffix}"))
}

fn refresh_persistent_index_checkpoint(
    path: &Path,
) -> Result<FileKernelPersistentIndexCheckpoint, KernelError> {
    let checkpoint = persistent_index_checkpoint_from_log(path)?;
    write_persistent_index_checkpoint(&persistent_index_path(path), &checkpoint)?;
    Ok(checkpoint)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FileKernelPersistentIndexCheckpointOpenState {
    present_on_open: bool,
    trusted_on_open: bool,
    rebuilt_on_open: bool,
}

fn ensure_persistent_index_checkpoint_for_open(
    path: &Path,
) -> Result<FileKernelPersistentIndexCheckpointOpenState, KernelError> {
    let index_path = persistent_index_path(path);
    let present_on_open = index_path.exists();
    match read_persistent_index_checkpoint(&index_path, path) {
        Ok(_checkpoint) => Ok(FileKernelPersistentIndexCheckpointOpenState {
            present_on_open,
            trusted_on_open: true,
            rebuilt_on_open: false,
        }),
        Err(KernelError::StoreIo | KernelError::StoreCorrupt) => {
            refresh_persistent_index_checkpoint(path)?;
            Ok(FileKernelPersistentIndexCheckpointOpenState {
                present_on_open,
                trusted_on_open: false,
                rebuilt_on_open: true,
            })
        }
        Err(error) => Err(error),
    }
}

fn persistent_index_checkpoint_or_rebuild(
    path: &Path,
) -> Result<FileKernelPersistentIndexCheckpoint, KernelError> {
    let index_path = persistent_index_path(path);
    match read_persistent_index_checkpoint(&index_path, path) {
        Ok(checkpoint) => Ok(checkpoint),
        Err(KernelError::StoreIo | KernelError::StoreCorrupt) => {
            refresh_persistent_index_checkpoint(path)
        }
        Err(error) => Err(error),
    }
}

fn read_persistent_index_checkpoint(
    index_path: &Path,
    log_path: &Path,
) -> Result<FileKernelPersistentIndexCheckpoint, KernelError> {
    let bytes = fs::read(index_path).map_err(|_error| KernelError::StoreIo)?;
    let checkpoint: FileKernelPersistentIndexCheckpoint =
        serde_json::from_slice(&bytes).map_err(|_error| KernelError::StoreCorrupt)?;
    let log_size_bytes = fs::metadata(log_path)
        .map_err(|_error| KernelError::StoreIo)?
        .len();
    if checkpoint.format == FILE_KERNEL_INDEX_FORMAT
        && checkpoint.version == FILE_KERNEL_INDEX_VERSION
        && checkpoint.log_size_bytes == log_size_bytes
        && checkpoint.cells.len() == checkpoint.cell_record_count
        && checkpoint.commit_manifests.len() == checkpoint.commit_manifest_record_count
        && checkpoint.revision_link_sources.len() == checkpoint.revision_link_record_count
        && checkpoint.revision_link_targets.len() == checkpoint.revision_link_record_count
        && checkpoint.revision_link_kinds.len() == checkpoint.revision_link_record_count
        && checkpoint.semantic_anchors.len() == checkpoint.semantic_anchor_address_count
        && checkpoint.commits.len() == checkpoint.commit_address_count
        && checkpoint.scopes.len() == checkpoint.scope_address_count
        && checkpoint.activations.len() == checkpoint.activation_address_count
        && checkpoint.lifecycle_stages.len() == checkpoint.lifecycle_stage_address_count
        && checkpoint.retention_policies.len() == checkpoint.retention_policy_address_count
        && checkpoint.use_policies.len() == checkpoint.use_policy_address_count
        && checkpoint.promotion_policies.len() == checkpoint.promotion_policy_address_count
        && checkpoint.projection_kinds.len() == checkpoint.projection_kind_address_count
        && checkpoint.native_uncertainties.len() == checkpoint.native_uncertainty_address_count
        && checkpoint.native_surprise_bits.len() == checkpoint.native_surprise_bits_address_count
        && checkpoint.native_saliences.len() == checkpoint.native_salience_address_count
        && checkpoint.native_context_affordances.len()
            == checkpoint.native_context_affordance_address_count
        && checkpoint.native_epistemic_pressures.len()
            == checkpoint.native_epistemic_pressure_address_count
        && checkpoint.trajectory_memory_strategies.len()
            == checkpoint.trajectory_memory_strategy_address_count
        && checkpoint.trajectory_memory_confidences.len()
            == checkpoint.trajectory_memory_confidence_address_count
        && checkpoint.context_gap_kinds.len() == checkpoint.context_gap_kind_address_count
        && checkpoint.context_gap_priorities.len() == checkpoint.context_gap_priority_address_count
        && checkpoint.invalidation_condition_kinds.len()
            == checkpoint.invalidation_condition_kind_address_count
        && checkpoint.invalidation_condition_priorities.len()
            == checkpoint.invalidation_condition_priority_address_count
        && checkpoint.epistemic_actions.len() == checkpoint.epistemic_action_address_count
        && checkpoint.epistemic_action_reasons.len()
            == checkpoint.epistemic_action_reason_address_count
        && checkpoint.answerability_questions.len()
            == checkpoint.answerability_question_address_count
        && checkpoint.evidence_sources.len() == checkpoint.evidence_source_address_count
        && checkpoint.max_evidence_confidences.len()
            == checkpoint.max_evidence_confidence_address_count
        && checkpoint.system_times.len() == checkpoint.system_time_address_count
        && checkpoint.valid_times.len() == checkpoint.valid_time_address_count
        && checkpoint.dependency_targets.len() == checkpoint.dependency_target_address_count
        && checkpoint.dependency_kinds.len() == checkpoint.dependency_kind_address_count
    {
        validate_persistent_index_checkpoint_addresses(log_path, &checkpoint)?;
        Ok(checkpoint)
    } else {
        Err(KernelError::StoreCorrupt)
    }
}

fn validate_persistent_index_checkpoint_addresses(
    path: &Path,
    checkpoint: &FileKernelPersistentIndexCheckpoint,
) -> Result<(), KernelError> {
    validate_persistent_index_checkpoint_primary_record_addresses(path, checkpoint)?;
    validate_persistent_index_checkpoint_revision_link_addresses(path, checkpoint)?;
    validate_persistent_index_checkpoint_secondary_cell_addresses(path, checkpoint)
}

fn validate_persistent_index_checkpoint_primary_record_addresses(
    path: &Path,
    checkpoint: &FileKernelPersistentIndexCheckpoint,
) -> Result<(), KernelError> {
    let mut cell_ids = HashSet::new();
    for address in &checkpoint.cells {
        read_cell_at_persistent_address_parts(path, address)?;
        if !cell_ids.insert(address.cell_id) {
            return Err(KernelError::StoreCorrupt);
        }
    }
    let mut commit_ids = HashSet::new();
    for address in &checkpoint.commit_manifests {
        read_commit_manifest_at_persistent_address_parts(path, address)?;
        if !commit_ids.insert(address.commit_id) {
            return Err(KernelError::StoreCorrupt);
        }
    }
    Ok(())
}

fn validate_persistent_index_checkpoint_revision_link_addresses(
    path: &Path,
    checkpoint: &FileKernelPersistentIndexCheckpoint,
) -> Result<(), KernelError> {
    let mut revision_link_sources = HashSet::new();
    for address in &checkpoint.revision_link_sources {
        read_revision_link_at_persistent_address_parts(path, address)?;
        if !revision_link_sources.insert((address.source, address.target, address.kind)) {
            return Err(KernelError::StoreCorrupt);
        }
    }
    let mut revision_link_targets = HashSet::new();
    for address in &checkpoint.revision_link_targets {
        read_revision_link_at_persistent_address_parts(path, address)?;
        if !revision_link_targets.insert((address.source, address.target, address.kind)) {
            return Err(KernelError::StoreCorrupt);
        }
    }
    let mut revision_link_kinds = HashSet::new();
    for address in &checkpoint.revision_link_kinds {
        read_revision_link_at_persistent_address_parts(path, address)?;
        if !revision_link_kinds.insert((address.source, address.target, address.kind)) {
            return Err(KernelError::StoreCorrupt);
        }
    }
    Ok(())
}

fn validate_persistent_index_checkpoint_secondary_cell_addresses(
    path: &Path,
    checkpoint: &FileKernelPersistentIndexCheckpoint,
) -> Result<(), KernelError> {
    for address in &checkpoint.semantic_anchors {
        let cell = read_cell_at_persistent_address_parts(path, address)?;
        if !cell
            .anchors
            .iter()
            .any(|anchor| anchor.as_str() == address.anchor.as_str())
        {
            return Err(KernelError::StoreCorrupt);
        }
    }
    for address in &checkpoint.commits {
        let cell = read_cell_at_persistent_address_parts(path, address)?;
        if cell.commit_id != address.commit_id {
            return Err(KernelError::StoreCorrupt);
        }
    }
    for address in &checkpoint.scopes {
        let cell = read_cell_at_persistent_address_parts(path, address)?;
        if cell.scope != address.scope {
            return Err(KernelError::StoreCorrupt);
        }
    }
    for address in &checkpoint.activations {
        let cell = read_cell_at_persistent_address_parts(path, address)?;
        if cell.activation != address.activation {
            return Err(KernelError::StoreCorrupt);
        }
    }
    for address in &checkpoint.lifecycle_stages {
        let cell = read_cell_at_persistent_address_parts(path, address)?;
        if cell.lifecycle_stage != address.lifecycle_stage {
            return Err(KernelError::StoreCorrupt);
        }
    }
    for address in &checkpoint.retention_policies {
        let cell = read_cell_at_persistent_address_parts(path, address)?;
        if cell.lifecycle_policy.retention != address.policy {
            return Err(KernelError::StoreCorrupt);
        }
    }
    for address in &checkpoint.use_policies {
        let cell = read_cell_at_persistent_address_parts(path, address)?;
        if cell.lifecycle_policy.use_policy != address.policy {
            return Err(KernelError::StoreCorrupt);
        }
    }
    for address in &checkpoint.promotion_policies {
        let cell = read_cell_at_persistent_address_parts(path, address)?;
        if cell.lifecycle_policy.promotion != address.policy {
            return Err(KernelError::StoreCorrupt);
        }
    }
    for address in &checkpoint.projection_kinds {
        let cell = read_cell_at_persistent_address_parts(path, address)?;
        if !cell
            .projections
            .iter()
            .any(|projection| projection.kind == address.projection_kind)
        {
            return Err(KernelError::StoreCorrupt);
        }
    }
    for address in &checkpoint.native_uncertainties {
        let cell = read_cell_at_persistent_address_parts(path, address)?;
        if confidence_microunits_ceil(cell.uncertainty.score.value())
            != address.uncertainty_microunits
        {
            return Err(KernelError::StoreCorrupt);
        }
    }
    for address in &checkpoint.native_surprise_bits {
        let cell = read_cell_at_persistent_address_parts(path, address)?;
        if surprise_microbits_ceil(cell.uncertainty.surprise_bits) != address.surprise_microbits {
            return Err(KernelError::StoreCorrupt);
        }
    }
    for address in &checkpoint.native_saliences {
        let cell = read_cell_at_persistent_address_parts(path, address)?;
        if salience_microunits_ceil(cell.attention.salience_score()) != address.salience_microunits
        {
            return Err(KernelError::StoreCorrupt);
        }
    }
    for address in &checkpoint.native_context_affordances {
        let cell = read_cell_at_persistent_address_parts(path, address)?;
        if salience_microunits_ceil(cell.context_affordance.context_affordance_score())
            != address.context_affordance_microunits
        {
            return Err(KernelError::StoreCorrupt);
        }
    }
    for address in &checkpoint.native_epistemic_pressures {
        let cell = read_cell_at_persistent_address_parts(path, address)?;
        if salience_microunits_ceil(cell.epistemic_pressure().checkout_pressure)
            != address.pressure_microunits
        {
            return Err(KernelError::StoreCorrupt);
        }
    }
    for address in &checkpoint.context_gap_kinds {
        let cell = read_cell_at_persistent_address_parts(path, address)?;
        if !cell.context_gaps.iter().any(|gap| gap.kind == address.kind) {
            return Err(KernelError::StoreCorrupt);
        }
    }
    for address in &checkpoint.context_gap_priorities {
        let cell = read_cell_at_persistent_address_parts(path, address)?;
        if !cell.context_gaps.iter().any(|gap| {
            confidence_microunits_ceil(gap.priority.value()) == address.priority_microunits
        }) {
            return Err(KernelError::StoreCorrupt);
        }
    }
    for address in &checkpoint.invalidation_condition_kinds {
        let cell = read_cell_at_persistent_address_parts(path, address)?;
        if !cell
            .invalidation_conditions
            .iter()
            .any(|condition| condition.kind == address.kind)
        {
            return Err(KernelError::StoreCorrupt);
        }
    }
    for address in &checkpoint.invalidation_condition_priorities {
        let cell = read_cell_at_persistent_address_parts(path, address)?;
        if !cell.invalidation_conditions.iter().any(|condition| {
            confidence_microunits_ceil(condition.priority.value()) == address.priority_microunits
        }) {
            return Err(KernelError::StoreCorrupt);
        }
    }
    for address in &checkpoint.trajectory_memory_strategies {
        let cell = read_cell_at_persistent_address_parts(path, address)?;
        if !cell
            .trajectory_memory
            .as_ref()
            .is_some_and(|memory| memory.checkout_strategy == address.strategy)
        {
            return Err(KernelError::StoreCorrupt);
        }
    }
    for address in &checkpoint.trajectory_memory_confidences {
        let cell = read_cell_at_persistent_address_parts(path, address)?;
        if !cell.trajectory_memory.as_ref().is_some_and(|memory| {
            confidence_microunits_ceil(memory.confidence.value()) == address.confidence_microunits
        }) {
            return Err(KernelError::StoreCorrupt);
        }
    }
    for address in &checkpoint.epistemic_actions {
        let cell = read_cell_at_persistent_address_parts(path, address)?;
        if cell.epistemic_action() != address.action {
            return Err(KernelError::StoreCorrupt);
        }
    }
    for address in &checkpoint.epistemic_action_reasons {
        let cell = read_cell_at_persistent_address_parts(path, address)?;
        if !cell.epistemic_action_reasons().contains(&address.reason) {
            return Err(KernelError::StoreCorrupt);
        }
    }
    for address in &checkpoint.answerability_questions {
        let cell = read_cell_at_persistent_address_parts(path, address)?;
        if !cell
            .answerability
            .questions()
            .iter()
            .any(|question| question.as_str() == address.question.as_str())
        {
            return Err(KernelError::StoreCorrupt);
        }
    }
    for address in &checkpoint.evidence_sources {
        let cell = read_cell_at_persistent_address_parts(path, address)?;
        if !cell
            .evidence
            .iter()
            .any(|evidence| evidence.source.as_str() == address.source.as_str())
        {
            return Err(KernelError::StoreCorrupt);
        }
    }
    for address in &checkpoint.max_evidence_confidences {
        let cell = read_cell_at_persistent_address_parts(path, address)?;
        if confidence_microunits_ceil(max_evidence_confidence(&cell))
            != address.confidence_microunits
        {
            return Err(KernelError::StoreCorrupt);
        }
    }
    for address in &checkpoint.system_times {
        let cell = read_cell_at_persistent_address_parts(path, address)?;
        if cell.system_time.from() != address.system_from {
            return Err(KernelError::StoreCorrupt);
        }
    }
    for address in &checkpoint.valid_times {
        let cell = read_cell_at_persistent_address_parts(path, address)?;
        if cell.valid_time.from() != address.valid_from {
            return Err(KernelError::StoreCorrupt);
        }
    }
    for address in &checkpoint.dependency_targets {
        let cell = read_cell_at_persistent_address_parts(path, address)?;
        if !cell
            .dependencies
            .iter()
            .any(|dependency| dependency.target == address.target)
        {
            return Err(KernelError::StoreCorrupt);
        }
    }
    for address in &checkpoint.dependency_kinds {
        let cell = read_cell_at_persistent_address_parts(path, address)?;
        if !cell
            .dependencies
            .iter()
            .any(|dependency| dependency.kind == address.kind)
        {
            return Err(KernelError::StoreCorrupt);
        }
    }
    Ok(())
}

fn persistent_index_checkpoint_from_log(
    path: &Path,
) -> Result<FileKernelPersistentIndexCheckpoint, KernelError> {
    let addresses = read_persistent_cell_addresses(path)?;
    Ok(FileKernelPersistentIndexCheckpoint {
        format: FILE_KERNEL_INDEX_FORMAT.to_string(),
        version: FILE_KERNEL_INDEX_VERSION,
        log_size_bytes: fs::metadata(path)
            .map_err(|_error| KernelError::StoreIo)?
            .len(),
        cell_record_count: addresses.cells.len(),
        commit_manifest_record_count: addresses.commit_manifests.len(),
        revision_link_record_count: addresses.revision_link_sources.len(),
        semantic_anchor_address_count: addresses.semantic_anchors.len(),
        commit_address_count: addresses.commits.len(),
        scope_address_count: addresses.scopes.len(),
        activation_address_count: addresses.activations.len(),
        lifecycle_stage_address_count: addresses.lifecycle_stages.len(),
        retention_policy_address_count: addresses.retention_policies.len(),
        use_policy_address_count: addresses.use_policies.len(),
        promotion_policy_address_count: addresses.promotion_policies.len(),
        projection_kind_address_count: addresses.projection_kinds.len(),
        native_uncertainty_address_count: addresses.native_uncertainties.len(),
        native_surprise_bits_address_count: addresses.native_surprise_bits.len(),
        native_salience_address_count: addresses.native_saliences.len(),
        native_context_affordance_address_count: addresses.native_context_affordances.len(),
        native_epistemic_pressure_address_count: addresses.native_epistemic_pressures.len(),
        trajectory_memory_strategy_address_count: addresses.trajectory_memory_strategies.len(),
        trajectory_memory_confidence_address_count: addresses.trajectory_memory_confidences.len(),
        context_gap_kind_address_count: addresses.context_gap_kinds.len(),
        context_gap_priority_address_count: addresses.context_gap_priorities.len(),
        invalidation_condition_kind_address_count: addresses.invalidation_condition_kinds.len(),
        invalidation_condition_priority_address_count: addresses
            .invalidation_condition_priorities
            .len(),
        epistemic_action_address_count: addresses.epistemic_actions.len(),
        epistemic_action_reason_address_count: addresses.epistemic_action_reasons.len(),
        answerability_question_address_count: addresses.answerability_questions.len(),
        evidence_source_address_count: addresses.evidence_sources.len(),
        max_evidence_confidence_address_count: addresses.max_evidence_confidences.len(),
        system_time_address_count: addresses.system_times.len(),
        valid_time_address_count: addresses.valid_times.len(),
        dependency_target_address_count: addresses.dependency_targets.len(),
        dependency_kind_address_count: addresses.dependency_kinds.len(),
        commit_manifests: addresses.commit_manifests,
        cells: addresses.cells,
        semantic_anchors: addresses.semantic_anchors,
        commits: addresses.commits,
        scopes: addresses.scopes,
        activations: addresses.activations,
        lifecycle_stages: addresses.lifecycle_stages,
        retention_policies: addresses.retention_policies,
        use_policies: addresses.use_policies,
        promotion_policies: addresses.promotion_policies,
        projection_kinds: addresses.projection_kinds,
        native_uncertainties: addresses.native_uncertainties,
        native_surprise_bits: addresses.native_surprise_bits,
        native_saliences: addresses.native_saliences,
        native_context_affordances: addresses.native_context_affordances,
        native_epistemic_pressures: addresses.native_epistemic_pressures,
        trajectory_memory_strategies: addresses.trajectory_memory_strategies,
        trajectory_memory_confidences: addresses.trajectory_memory_confidences,
        context_gap_kinds: addresses.context_gap_kinds,
        context_gap_priorities: addresses.context_gap_priorities,
        invalidation_condition_kinds: addresses.invalidation_condition_kinds,
        invalidation_condition_priorities: addresses.invalidation_condition_priorities,
        epistemic_actions: addresses.epistemic_actions,
        epistemic_action_reasons: addresses.epistemic_action_reasons,
        answerability_questions: addresses.answerability_questions,
        evidence_sources: addresses.evidence_sources,
        max_evidence_confidences: addresses.max_evidence_confidences,
        system_times: addresses.system_times,
        valid_times: addresses.valid_times,
        dependency_targets: addresses.dependency_targets,
        dependency_kinds: addresses.dependency_kinds,
        revision_link_sources: addresses.revision_link_sources,
        revision_link_targets: addresses.revision_link_targets,
        revision_link_kinds: addresses.revision_link_kinds,
    })
}

fn read_persistent_cell_addresses(
    path: &Path,
) -> Result<FileKernelPersistentAddressIndexes, KernelError> {
    let file = File::open(path).map_err(|_error| KernelError::StoreIo)?;
    let mut reader = BufReader::new(file);
    let mut addresses = FileKernelPersistentAddressIndexes::default();
    let mut offset = 0_u64;
    let mut line = String::new();

    loop {
        line.clear();
        let length = reader
            .read_line(&mut line)
            .map_err(|_error| KernelError::StoreIo)? as u64;
        if length == 0 {
            break;
        }

        if line.trim().is_empty() {
            offset += length;
            continue;
        }

        match serde_json::from_str::<FileKernelRecord>(&line) {
            Ok(FileKernelRecord::Cell { cell, checksum }) => {
                validate_file_record_checksum(cell.as_ref(), checksum.as_deref())
                    .map_err(|_error| KernelError::StoreCorrupt)?;
                let checksum = checksum.unwrap_or(file_record_checksum(cell.as_ref())?);
                addresses.cells.push(FileKernelPersistentCellAddress {
                    cell_id: cell.id,
                    offset,
                    length,
                    checksum: checksum.clone(),
                });
                addresses.commits.push(FileKernelPersistentCommitAddress {
                    commit_id: cell.commit_id,
                    cell_id: cell.id,
                    offset,
                    length,
                    checksum: checksum.clone(),
                });
                addresses.scopes.push(FileKernelPersistentScopeAddress {
                    scope: cell.scope.clone(),
                    cell_id: cell.id,
                    offset,
                    length,
                    checksum: checksum.clone(),
                });
                addresses
                    .system_times
                    .push(FileKernelPersistentSystemTimeAddress {
                        system_from: cell.system_time.from(),
                        cell_id: cell.id,
                        offset,
                        length,
                        checksum: checksum.clone(),
                    });
                addresses
                    .valid_times
                    .push(FileKernelPersistentValidTimeAddress {
                        valid_from: cell.valid_time.from(),
                        cell_id: cell.id,
                        offset,
                        length,
                        checksum: checksum.clone(),
                    });
                for dependency in &cell.dependencies {
                    addresses.dependency_targets.push(
                        FileKernelPersistentDependencyTargetAddress {
                            target: dependency.target,
                            cell_id: cell.id,
                            offset,
                            length,
                            checksum: checksum.clone(),
                        },
                    );
                    addresses
                        .dependency_kinds
                        .push(FileKernelPersistentDependencyKindAddress {
                            kind: dependency.kind,
                            cell_id: cell.id,
                            offset,
                            length,
                            checksum: checksum.clone(),
                        });
                }
                addresses.max_evidence_confidences.push(
                    FileKernelPersistentMaxEvidenceConfidenceAddress {
                        confidence_microunits: confidence_microunits_ceil(max_evidence_confidence(
                            cell.as_ref(),
                        )),
                        cell_id: cell.id,
                        offset,
                        length,
                        checksum: checksum.clone(),
                    },
                );
                addresses
                    .activations
                    .push(FileKernelPersistentActivationAddress {
                        activation: cell.activation,
                        cell_id: cell.id,
                        offset,
                        length,
                        checksum: checksum.clone(),
                    });
                addresses
                    .lifecycle_stages
                    .push(FileKernelPersistentLifecycleStageAddress {
                        lifecycle_stage: cell.lifecycle_stage,
                        cell_id: cell.id,
                        offset,
                        length,
                        checksum: checksum.clone(),
                    });
                addresses
                    .retention_policies
                    .push(FileKernelPersistentRetentionPolicyAddress {
                        policy: cell.lifecycle_policy.retention,
                        cell_id: cell.id,
                        offset,
                        length,
                        checksum: checksum.clone(),
                    });
                addresses
                    .use_policies
                    .push(FileKernelPersistentUsePolicyAddress {
                        policy: cell.lifecycle_policy.use_policy,
                        cell_id: cell.id,
                        offset,
                        length,
                        checksum: checksum.clone(),
                    });
                addresses
                    .promotion_policies
                    .push(FileKernelPersistentPromotionPolicyAddress {
                        policy: cell.lifecycle_policy.promotion,
                        cell_id: cell.id,
                        offset,
                        length,
                        checksum: checksum.clone(),
                    });
                for projection in &cell.projections {
                    addresses
                        .projection_kinds
                        .push(FileKernelPersistentProjectionKindAddress {
                            projection_kind: projection.kind,
                            cell_id: cell.id,
                            offset,
                            length,
                            checksum: checksum.clone(),
                        });
                }
                addresses
                    .native_uncertainties
                    .push(FileKernelPersistentNativeUncertaintyAddress {
                        uncertainty_microunits: confidence_microunits_ceil(
                            cell.uncertainty.score.value(),
                        ),
                        cell_id: cell.id,
                        offset,
                        length,
                        checksum: checksum.clone(),
                    });
                addresses.native_surprise_bits.push(
                    FileKernelPersistentNativeSurpriseBitsAddress {
                        surprise_microbits: surprise_microbits_ceil(cell.uncertainty.surprise_bits),
                        cell_id: cell.id,
                        offset,
                        length,
                        checksum: checksum.clone(),
                    },
                );
                addresses
                    .native_saliences
                    .push(FileKernelPersistentNativeSalienceAddress {
                        salience_microunits: salience_microunits_ceil(
                            cell.attention.salience_score(),
                        ),
                        cell_id: cell.id,
                        offset,
                        length,
                        checksum: checksum.clone(),
                    });
                addresses.native_context_affordances.push(
                    FileKernelPersistentNativeContextAffordanceAddress {
                        context_affordance_microunits: salience_microunits_ceil(
                            cell.context_affordance.context_affordance_score(),
                        ),
                        cell_id: cell.id,
                        offset,
                        length,
                        checksum: checksum.clone(),
                    },
                );
                addresses.native_epistemic_pressures.push(
                    FileKernelPersistentNativeEpistemicPressureAddress {
                        pressure_microunits: salience_microunits_ceil(
                            cell.epistemic_pressure().checkout_pressure,
                        ),
                        cell_id: cell.id,
                        offset,
                        length,
                        checksum: checksum.clone(),
                    },
                );
                for gap in &cell.context_gaps {
                    addresses
                        .context_gap_kinds
                        .push(FileKernelPersistentContextGapKindAddress {
                            kind: gap.kind,
                            cell_id: cell.id,
                            offset,
                            length,
                            checksum: checksum.clone(),
                        });
                    addresses.context_gap_priorities.push(
                        FileKernelPersistentContextGapPriorityAddress {
                            priority_microunits: confidence_microunits_ceil(gap.priority.value()),
                            cell_id: cell.id,
                            offset,
                            length,
                            checksum: checksum.clone(),
                        },
                    );
                }
                for condition in &cell.invalidation_conditions {
                    addresses.invalidation_condition_kinds.push(
                        FileKernelPersistentInvalidationConditionKindAddress {
                            kind: condition.kind,
                            cell_id: cell.id,
                            offset,
                            length,
                            checksum: checksum.clone(),
                        },
                    );
                    addresses.invalidation_condition_priorities.push(
                        FileKernelPersistentInvalidationConditionPriorityAddress {
                            priority_microunits: confidence_microunits_ceil(
                                condition.priority.value(),
                            ),
                            cell_id: cell.id,
                            offset,
                            length,
                            checksum: checksum.clone(),
                        },
                    );
                }
                if let Some(trajectory_memory) = &cell.trajectory_memory {
                    addresses.trajectory_memory_strategies.push(
                        FileKernelPersistentTrajectoryMemoryStrategyAddress {
                            strategy: trajectory_memory.checkout_strategy,
                            cell_id: cell.id,
                            offset,
                            length,
                            checksum: checksum.clone(),
                        },
                    );
                    addresses.trajectory_memory_confidences.push(
                        FileKernelPersistentTrajectoryMemoryConfidenceAddress {
                            confidence_microunits: confidence_microunits_ceil(
                                trajectory_memory.confidence.value(),
                            ),
                            cell_id: cell.id,
                            offset,
                            length,
                            checksum: checksum.clone(),
                        },
                    );
                }
                addresses
                    .epistemic_actions
                    .push(FileKernelPersistentEpistemicActionAddress {
                        action: cell.epistemic_action(),
                        cell_id: cell.id,
                        offset,
                        length,
                        checksum: checksum.clone(),
                    });
                for reason in cell.epistemic_action_reasons() {
                    addresses.epistemic_action_reasons.push(
                        FileKernelPersistentEpistemicActionReasonAddress {
                            reason,
                            cell_id: cell.id,
                            offset,
                            length,
                            checksum: checksum.clone(),
                        },
                    );
                }
                for question in cell.answerability.questions() {
                    addresses.answerability_questions.push(
                        FileKernelPersistentAnswerabilityQuestionAddress {
                            question: question.clone(),
                            cell_id: cell.id,
                            offset,
                            length,
                            checksum: checksum.clone(),
                        },
                    );
                }
                for evidence in &cell.evidence {
                    addresses
                        .evidence_sources
                        .push(FileKernelPersistentEvidenceSourceAddress {
                            source: evidence.source.as_str().to_string(),
                            cell_id: cell.id,
                            offset,
                            length,
                            checksum: checksum.clone(),
                        });
                }
                for anchor in &cell.anchors {
                    addresses
                        .semantic_anchors
                        .push(FileKernelPersistentSemanticAnchorAddress {
                            anchor: anchor.as_str().to_string(),
                            cell_id: cell.id,
                            offset,
                            length,
                            checksum: checksum.clone(),
                        });
                }
            }
            Ok(FileKernelRecord::RevisionLink {
                revision_link,
                checksum,
            }) => {
                validate_file_record_checksum(&revision_link, checksum.as_deref())
                    .map_err(|_error| KernelError::StoreCorrupt)?;
                let checksum = checksum.unwrap_or(file_record_checksum(&revision_link)?);
                addresses.revision_link_sources.push(
                    FileKernelPersistentRevisionLinkSourceAddress {
                        source: revision_link.source,
                        target: revision_link.target,
                        kind: revision_link.kind,
                        offset,
                        length,
                        checksum,
                    },
                );
                addresses.revision_link_targets.push(
                    FileKernelPersistentRevisionLinkTargetAddress {
                        target: revision_link.target,
                        source: revision_link.source,
                        kind: revision_link.kind,
                        offset,
                        length,
                        checksum: file_record_checksum(&revision_link)?,
                    },
                );
                addresses
                    .revision_link_kinds
                    .push(FileKernelPersistentRevisionLinkKindAddress {
                        kind: revision_link.kind,
                        source: revision_link.source,
                        target: revision_link.target,
                        offset,
                        length,
                        checksum: file_record_checksum(&revision_link)?,
                    });
            }
            Ok(FileKernelRecord::Commit { manifest, checksum }) => {
                validate_file_record_checksum(&manifest, checksum.as_deref())
                    .map_err(|_error| KernelError::StoreCorrupt)?;
                let checksum = checksum.unwrap_or(file_record_checksum(&manifest)?);
                addresses
                    .commit_manifests
                    .push(FileKernelPersistentCommitManifestAddress {
                        commit_id: manifest.commit_id,
                        offset,
                        length,
                        checksum,
                    });
            }
            Ok(FileKernelRecord::Header { .. }) => {}
            Err(_record_error) => {
                let cell: StateCell =
                    serde_json::from_str(&line).map_err(|_error| KernelError::StoreCorrupt)?;
                let checksum = file_record_checksum(&cell)?;
                addresses.cells.push(FileKernelPersistentCellAddress {
                    cell_id: cell.id,
                    offset,
                    length,
                    checksum: checksum.clone(),
                });
                addresses.commits.push(FileKernelPersistentCommitAddress {
                    commit_id: cell.commit_id,
                    cell_id: cell.id,
                    offset,
                    length,
                    checksum: checksum.clone(),
                });
                addresses.scopes.push(FileKernelPersistentScopeAddress {
                    scope: cell.scope.clone(),
                    cell_id: cell.id,
                    offset,
                    length,
                    checksum: checksum.clone(),
                });
                addresses
                    .system_times
                    .push(FileKernelPersistentSystemTimeAddress {
                        system_from: cell.system_time.from(),
                        cell_id: cell.id,
                        offset,
                        length,
                        checksum: checksum.clone(),
                    });
                addresses
                    .valid_times
                    .push(FileKernelPersistentValidTimeAddress {
                        valid_from: cell.valid_time.from(),
                        cell_id: cell.id,
                        offset,
                        length,
                        checksum: checksum.clone(),
                    });
                for dependency in &cell.dependencies {
                    addresses.dependency_targets.push(
                        FileKernelPersistentDependencyTargetAddress {
                            target: dependency.target,
                            cell_id: cell.id,
                            offset,
                            length,
                            checksum: checksum.clone(),
                        },
                    );
                    addresses
                        .dependency_kinds
                        .push(FileKernelPersistentDependencyKindAddress {
                            kind: dependency.kind,
                            cell_id: cell.id,
                            offset,
                            length,
                            checksum: checksum.clone(),
                        });
                }
                addresses.max_evidence_confidences.push(
                    FileKernelPersistentMaxEvidenceConfidenceAddress {
                        confidence_microunits: confidence_microunits_ceil(max_evidence_confidence(
                            &cell,
                        )),
                        cell_id: cell.id,
                        offset,
                        length,
                        checksum: checksum.clone(),
                    },
                );
                addresses
                    .activations
                    .push(FileKernelPersistentActivationAddress {
                        activation: cell.activation,
                        cell_id: cell.id,
                        offset,
                        length,
                        checksum: checksum.clone(),
                    });
                addresses
                    .lifecycle_stages
                    .push(FileKernelPersistentLifecycleStageAddress {
                        lifecycle_stage: cell.lifecycle_stage,
                        cell_id: cell.id,
                        offset,
                        length,
                        checksum: checksum.clone(),
                    });
                addresses
                    .retention_policies
                    .push(FileKernelPersistentRetentionPolicyAddress {
                        policy: cell.lifecycle_policy.retention,
                        cell_id: cell.id,
                        offset,
                        length,
                        checksum: checksum.clone(),
                    });
                addresses
                    .use_policies
                    .push(FileKernelPersistentUsePolicyAddress {
                        policy: cell.lifecycle_policy.use_policy,
                        cell_id: cell.id,
                        offset,
                        length,
                        checksum: checksum.clone(),
                    });
                addresses
                    .promotion_policies
                    .push(FileKernelPersistentPromotionPolicyAddress {
                        policy: cell.lifecycle_policy.promotion,
                        cell_id: cell.id,
                        offset,
                        length,
                        checksum: checksum.clone(),
                    });
                for projection in &cell.projections {
                    addresses
                        .projection_kinds
                        .push(FileKernelPersistentProjectionKindAddress {
                            projection_kind: projection.kind,
                            cell_id: cell.id,
                            offset,
                            length,
                            checksum: checksum.clone(),
                        });
                }
                addresses
                    .native_uncertainties
                    .push(FileKernelPersistentNativeUncertaintyAddress {
                        uncertainty_microunits: confidence_microunits_ceil(
                            cell.uncertainty.score.value(),
                        ),
                        cell_id: cell.id,
                        offset,
                        length,
                        checksum: checksum.clone(),
                    });
                addresses.native_surprise_bits.push(
                    FileKernelPersistentNativeSurpriseBitsAddress {
                        surprise_microbits: surprise_microbits_ceil(cell.uncertainty.surprise_bits),
                        cell_id: cell.id,
                        offset,
                        length,
                        checksum: checksum.clone(),
                    },
                );
                addresses
                    .native_saliences
                    .push(FileKernelPersistentNativeSalienceAddress {
                        salience_microunits: salience_microunits_ceil(
                            cell.attention.salience_score(),
                        ),
                        cell_id: cell.id,
                        offset,
                        length,
                        checksum: checksum.clone(),
                    });
                addresses.native_context_affordances.push(
                    FileKernelPersistentNativeContextAffordanceAddress {
                        context_affordance_microunits: salience_microunits_ceil(
                            cell.context_affordance.context_affordance_score(),
                        ),
                        cell_id: cell.id,
                        offset,
                        length,
                        checksum: checksum.clone(),
                    },
                );
                addresses.native_epistemic_pressures.push(
                    FileKernelPersistentNativeEpistemicPressureAddress {
                        pressure_microunits: salience_microunits_ceil(
                            cell.epistemic_pressure().checkout_pressure,
                        ),
                        cell_id: cell.id,
                        offset,
                        length,
                        checksum: checksum.clone(),
                    },
                );
                for gap in &cell.context_gaps {
                    addresses
                        .context_gap_kinds
                        .push(FileKernelPersistentContextGapKindAddress {
                            kind: gap.kind,
                            cell_id: cell.id,
                            offset,
                            length,
                            checksum: checksum.clone(),
                        });
                    addresses.context_gap_priorities.push(
                        FileKernelPersistentContextGapPriorityAddress {
                            priority_microunits: confidence_microunits_ceil(gap.priority.value()),
                            cell_id: cell.id,
                            offset,
                            length,
                            checksum: checksum.clone(),
                        },
                    );
                }
                for condition in &cell.invalidation_conditions {
                    addresses.invalidation_condition_kinds.push(
                        FileKernelPersistentInvalidationConditionKindAddress {
                            kind: condition.kind,
                            cell_id: cell.id,
                            offset,
                            length,
                            checksum: checksum.clone(),
                        },
                    );
                    addresses.invalidation_condition_priorities.push(
                        FileKernelPersistentInvalidationConditionPriorityAddress {
                            priority_microunits: confidence_microunits_ceil(
                                condition.priority.value(),
                            ),
                            cell_id: cell.id,
                            offset,
                            length,
                            checksum: checksum.clone(),
                        },
                    );
                }
                if let Some(trajectory_memory) = &cell.trajectory_memory {
                    addresses.trajectory_memory_strategies.push(
                        FileKernelPersistentTrajectoryMemoryStrategyAddress {
                            strategy: trajectory_memory.checkout_strategy,
                            cell_id: cell.id,
                            offset,
                            length,
                            checksum: checksum.clone(),
                        },
                    );
                    addresses.trajectory_memory_confidences.push(
                        FileKernelPersistentTrajectoryMemoryConfidenceAddress {
                            confidence_microunits: confidence_microunits_ceil(
                                trajectory_memory.confidence.value(),
                            ),
                            cell_id: cell.id,
                            offset,
                            length,
                            checksum: checksum.clone(),
                        },
                    );
                }
                addresses
                    .epistemic_actions
                    .push(FileKernelPersistentEpistemicActionAddress {
                        action: cell.epistemic_action(),
                        cell_id: cell.id,
                        offset,
                        length,
                        checksum: checksum.clone(),
                    });
                for reason in cell.epistemic_action_reasons() {
                    addresses.epistemic_action_reasons.push(
                        FileKernelPersistentEpistemicActionReasonAddress {
                            reason,
                            cell_id: cell.id,
                            offset,
                            length,
                            checksum: checksum.clone(),
                        },
                    );
                }
                for question in cell.answerability.questions() {
                    addresses.answerability_questions.push(
                        FileKernelPersistentAnswerabilityQuestionAddress {
                            question: question.clone(),
                            cell_id: cell.id,
                            offset,
                            length,
                            checksum: checksum.clone(),
                        },
                    );
                }
                for evidence in &cell.evidence {
                    addresses
                        .evidence_sources
                        .push(FileKernelPersistentEvidenceSourceAddress {
                            source: evidence.source.as_str().to_string(),
                            cell_id: cell.id,
                            offset,
                            length,
                            checksum: checksum.clone(),
                        });
                }
                for anchor in &cell.anchors {
                    addresses
                        .semantic_anchors
                        .push(FileKernelPersistentSemanticAnchorAddress {
                            anchor: anchor.as_str().to_string(),
                            cell_id: cell.id,
                            offset,
                            length,
                            checksum: checksum.clone(),
                        });
                }
            }
        }

        offset += length;
    }

    Ok(addresses)
}

fn read_commit_manifest_at_persistent_address_parts(
    path: &Path,
    address: &impl FileKernelPersistentCommitManifestAddressParts,
) -> Result<CommitManifest, KernelError> {
    let mut file = File::open(path).map_err(|_error| KernelError::StoreIo)?;
    file.seek(SeekFrom::Start(address.offset()))
        .map_err(|_error| KernelError::StoreIo)?;
    let mut record_bytes = vec![0; address.length() as usize];
    file.read_exact(&mut record_bytes)
        .map_err(|_error| KernelError::StoreIo)?;

    let manifest = match serde_json::from_slice::<FileKernelRecord>(&record_bytes) {
        Ok(FileKernelRecord::Commit { manifest, checksum }) => {
            validate_file_record_checksum(&manifest, checksum.as_deref())?;
            let record_checksum = checksum.unwrap_or(file_record_checksum(&manifest)?);
            if record_checksum != address.checksum() {
                return Err(KernelError::StoreCorrupt);
            }
            manifest
        }
        Ok(
            FileKernelRecord::Header { .. }
            | FileKernelRecord::Cell { .. }
            | FileKernelRecord::RevisionLink { .. },
        )
        | Err(_) => return Err(KernelError::StoreCorrupt),
    };

    if manifest.commit_id == address.commit_id() {
        Ok(manifest)
    } else {
        Err(KernelError::StoreCorrupt)
    }
}

fn read_cell_at_persistent_address(
    path: &Path,
    address: &FileKernelPersistentCellAddress,
) -> Result<StateCell, KernelError> {
    read_cell_at_persistent_address_parts(path, address)
}

fn read_cell_at_persistent_address_parts(
    path: &Path,
    address: &impl FileKernelPersistentCellAddressParts,
) -> Result<StateCell, KernelError> {
    let mut file = File::open(path).map_err(|_error| KernelError::StoreIo)?;
    file.seek(SeekFrom::Start(address.offset()))
        .map_err(|_error| KernelError::StoreIo)?;
    let mut record_bytes = vec![0; address.length() as usize];
    file.read_exact(&mut record_bytes)
        .map_err(|_error| KernelError::StoreIo)?;

    let cell = match serde_json::from_slice::<FileKernelRecord>(&record_bytes) {
        Ok(FileKernelRecord::Cell { cell, checksum }) => {
            validate_file_record_checksum(cell.as_ref(), checksum.as_deref())?;
            let record_checksum = checksum.unwrap_or(file_record_checksum(cell.as_ref())?);
            if record_checksum != address.checksum() {
                return Err(KernelError::StoreCorrupt);
            }
            *cell
        }
        Ok(
            FileKernelRecord::Header { .. }
            | FileKernelRecord::Commit { .. }
            | FileKernelRecord::RevisionLink { .. },
        ) => return Err(KernelError::StoreCorrupt),
        Err(_record_error) => {
            let cell: StateCell = serde_json::from_slice(&record_bytes)
                .map_err(|_error| KernelError::StoreCorrupt)?;
            if file_record_checksum(&cell)? != address.checksum() {
                return Err(KernelError::StoreCorrupt);
            }
            cell
        }
    };

    if cell.id == address.cell_id() {
        Ok(cell)
    } else {
        Err(KernelError::StoreCorrupt)
    }
}

fn file_kernel_index_from_persistent_cell_addresses<'a>(
    path: &Path,
    addresses: impl IntoIterator<Item = &'a FileKernelPersistentCellAddress>,
) -> Result<FileKernelIndex, KernelError> {
    let mut index = FileKernelIndex::default();
    for address in addresses {
        index.insert(read_cell_at_persistent_address_parts(path, address)?)?;
    }
    Ok(index)
}

fn persistent_index_contains_cell_id(
    path: &Path,
    cell_id: StateCellId,
) -> Result<bool, KernelError> {
    let checkpoint = persistent_index_checkpoint_or_rebuild(path)?;
    Ok(checkpoint
        .cells
        .iter()
        .any(|address| address.cell_id == cell_id))
}

fn persistent_index_contains_commit_id(
    path: &Path,
    commit_id: CommitId,
) -> Result<bool, KernelError> {
    let checkpoint = persistent_index_checkpoint_or_rebuild(path)?;
    Ok(checkpoint
        .commit_manifests
        .iter()
        .any(|address| address.commit_id == commit_id))
}

fn persistent_index_contains_revision_link(
    path: &Path,
    revision_link: &RevisionLinkRecord,
) -> Result<bool, KernelError> {
    let checkpoint = persistent_index_checkpoint_or_rebuild(path)?;
    Ok(checkpoint.revision_link_sources.iter().any(|address| {
        address.source == revision_link.source
            && address.target == revision_link.target
            && address.kind == revision_link.kind
    }))
}

fn read_revision_link_at_persistent_address_parts(
    path: &Path,
    address: &impl FileKernelPersistentRevisionLinkAddressParts,
) -> Result<RevisionLinkRecord, KernelError> {
    let mut file = File::open(path).map_err(|_error| KernelError::StoreIo)?;
    file.seek(SeekFrom::Start(address.offset()))
        .map_err(|_error| KernelError::StoreIo)?;
    let mut record_bytes = vec![0; address.length() as usize];
    file.read_exact(&mut record_bytes)
        .map_err(|_error| KernelError::StoreIo)?;

    let revision_link = match serde_json::from_slice::<FileKernelRecord>(&record_bytes) {
        Ok(FileKernelRecord::RevisionLink {
            revision_link,
            checksum,
        }) => {
            validate_file_record_checksum(&revision_link, checksum.as_deref())?;
            let record_checksum = checksum.unwrap_or(file_record_checksum(&revision_link)?);
            if record_checksum != address.checksum() {
                return Err(KernelError::StoreCorrupt);
            }
            revision_link
        }
        Ok(
            FileKernelRecord::Header { .. }
            | FileKernelRecord::Cell { .. }
            | FileKernelRecord::Commit { .. },
        )
        | Err(_) => return Err(KernelError::StoreCorrupt),
    };

    if revision_link.source == address.source()
        && revision_link.target == address.target()
        && revision_link.kind == address.kind()
    {
        Ok(revision_link)
    } else {
        Err(KernelError::StoreCorrupt)
    }
}

fn write_persistent_index_checkpoint(
    path: &Path,
    checkpoint: &FileKernelPersistentIndexCheckpoint,
) -> Result<(), KernelError> {
    let temp_path = persistent_index_temp_path(path);
    {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp_path)
            .map_err(|_error| KernelError::StoreIo)?;
        let encoded =
            serde_json::to_vec_pretty(checkpoint).map_err(|_error| KernelError::StoreCorrupt)?;
        write_all_durable(&mut file, &encoded)?;
    }
    fs::rename(&temp_path, path).map_err(|_error| KernelError::StoreIo)?;
    sync_parent_directory(path)
}

fn ensure_file_header(path: &Path) -> Result<(), KernelError> {
    if fs::metadata(path)
        .map_err(|_error| KernelError::StoreIo)?
        .len()
        != 0
    {
        return Ok(());
    }

    let header = FileKernelHeader::current();
    let encoded = serde_json::to_string(&FileKernelRecord::Header {
        format: header.format,
        version: header.version,
    })
    .map_err(|_error| KernelError::StoreCorrupt)?;
    let mut file = OpenOptions::new()
        .append(true)
        .open(path)
        .map_err(|_error| KernelError::StoreIo)?;
    let record = format!("{encoded}\n");
    write_all_durable(&mut file, record.as_bytes())
}

fn write_all_durable(file: &mut File, bytes: &[u8]) -> Result<(), KernelError> {
    file.write_all(bytes)
        .map_err(|_error| KernelError::StoreIo)?;
    file.flush().map_err(|_error| KernelError::StoreIo)?;
    file.sync_all().map_err(|_error| KernelError::StoreIo)
}

fn sync_parent_directory(path: &Path) -> Result<(), KernelError> {
    let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    else {
        return Ok(());
    };
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|_error| KernelError::StoreIo)
}

fn file_record_checksum<T: serde::Serialize>(payload: &T) -> Result<String, KernelError> {
    let bytes = serde_json::to_vec(payload).map_err(|_error| KernelError::StoreCorrupt)?;
    let mut hash = FNV1A64_OFFSET;
    for byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV1A64_PRIME);
    }
    Ok(format!("{FILE_KERNEL_CHECKSUM_ALGORITHM}:{hash:016x}"))
}

fn validate_file_record_checksum<T: serde::Serialize>(
    payload: &T,
    checksum: Option<&str>,
) -> Result<(), KernelError> {
    if let Some(checksum) = checksum {
        if file_record_checksum(payload)? != checksum {
            return Err(KernelError::StoreCorrupt);
        }
    }
    Ok(())
}

fn corrupt_record(line: usize) -> KernelError {
    KernelError::StoreCorruptRecord { line }
}

fn encode_canonical_log<'a>(
    cells: impl IntoIterator<Item = &'a StateCell>,
    manifests: impl IntoIterator<Item = &'a CommitManifest>,
    revision_links: impl IntoIterator<Item = &'a RevisionLinkRecord>,
) -> Result<String, KernelError> {
    let header = FileKernelHeader::current();
    let mut encoded = String::new();
    encoded.push_str(
        &serde_json::to_string(&FileKernelRecord::Header {
            format: header.format,
            version: header.version,
        })
        .map_err(|_error| KernelError::StoreCorrupt)?,
    );
    encoded.push('\n');

    for cell in cells {
        encoded.push_str(
            &serde_json::to_string(&FileKernelRecord::Cell {
                cell: Box::new(cell.clone()),
                checksum: Some(file_record_checksum(cell)?),
            })
            .map_err(|_error| KernelError::StoreCorrupt)?,
        );
        encoded.push('\n');
    }

    for manifest in manifests {
        encoded.push_str(
            &serde_json::to_string(&FileKernelRecord::Commit {
                manifest: manifest.clone(),
                checksum: Some(file_record_checksum(manifest)?),
            })
            .map_err(|_error| KernelError::StoreCorrupt)?,
        );
        encoded.push('\n');
    }

    for revision_link in revision_links {
        encoded.push_str(
            &serde_json::to_string(&FileKernelRecord::RevisionLink {
                revision_link: revision_link.clone(),
                checksum: Some(file_record_checksum(revision_link)?),
            })
            .map_err(|_error| KernelError::StoreCorrupt)?,
        );
        encoded.push('\n');
    }

    Ok(encoded)
}

fn read_log_from_path(path: &Path) -> Result<FileKernelLog, KernelError> {
    let file = File::open(path).map_err(|_error| KernelError::StoreIo)?;
    let reader = BufReader::new(file);
    let mut log = FileKernelLog::default();
    let mut seen_header = false;
    let mut seen_data = false;

    for (line_index, line) in reader.lines().enumerate() {
        let line_number = line_index + 1;
        let line = line.map_err(|_error| KernelError::StoreIo)?;
        if line.trim().is_empty() {
            continue;
        }

        match serde_json::from_str::<FileKernelRecord>(&line) {
            Ok(FileKernelRecord::Header { format, version }) => {
                if seen_header || seen_data {
                    return Err(corrupt_record(line_number));
                }
                FileKernelHeader { format, version }
                    .validate()
                    .map_err(|_error| corrupt_record(line_number))?;
                log.has_header = true;
                log.health.has_header = true;
                log.health.compaction_recommended =
                    log.health.legacy_raw_cells > 0 || log.health.checksum_free_records > 0;
                seen_header = true;
            }
            Ok(FileKernelRecord::Cell { cell, checksum }) => {
                seen_data = true;
                record_file_health(&mut log.health, checksum.is_some());
                validate_file_record_checksum(cell.as_ref(), checksum.as_deref())
                    .map_err(|_error| corrupt_record(line_number))?;
                log.cells.push(*cell);
            }
            Ok(FileKernelRecord::Commit { manifest, checksum }) => {
                seen_data = true;
                record_file_health(&mut log.health, checksum.is_some());
                validate_file_record_checksum(&manifest, checksum.as_deref())
                    .map_err(|_error| corrupt_record(line_number))?;
                log.explicit_manifests.push(manifest);
            }
            Ok(FileKernelRecord::RevisionLink {
                revision_link,
                checksum,
            }) => {
                seen_data = true;
                record_file_health(&mut log.health, checksum.is_some());
                validate_file_record_checksum(&revision_link, checksum.as_deref())
                    .map_err(|_error| corrupt_record(line_number))?;
                log.revision_links.push(revision_link);
            }
            Err(_record_error) => {
                seen_data = true;
                log.health.legacy_raw_cells += 1;
                log.health.compaction_recommended = true;
                log.cells.push(
                    serde_json::from_str(&line)
                        .map_err(|_cell_error| corrupt_record(line_number))?,
                );
            }
        }
    }

    Ok(log)
}

fn record_file_health(health: &mut FileKernelHealth, has_checksum: bool) {
    if has_checksum {
        health.canonical_records += 1;
    } else {
        health.checksum_free_records += 1;
        health.compaction_recommended = true;
    }
}

fn max_evidence_confidence(cell: &StateCell) -> f32 {
    cell.evidence
        .iter()
        .map(|evidence| evidence.confidence.value())
        .fold(0.0, f32::max)
}

fn confidence_microunits_floor(confidence: Confidence) -> u32 {
    confidence_microunits(confidence.value(), f32::floor)
}

fn confidence_microunits_ceil(confidence: f32) -> u32 {
    confidence_microunits(confidence, f32::ceil)
}

fn confidence_microunits(confidence: f32, round: fn(f32) -> f32) -> u32 {
    round(confidence.clamp(0.0, 1.0) * CONFIDENCE_INDEX_MICRO_UNITS) as u32
}

fn salience_microunits_floor(salience: f32) -> u32 {
    salience_microunits(salience, f32::floor)
}

fn salience_microunits_ceil(salience: f32) -> u32 {
    salience_microunits(salience, f32::ceil)
}

fn salience_microunits(salience: f32, round: fn(f32) -> f32) -> u32 {
    round(salience.clamp(0.0, 1.0) * CONFIDENCE_INDEX_MICRO_UNITS) as u32
}

fn surprise_microbits_floor(surprise_bits: f32) -> u64 {
    surprise_microbits(surprise_bits, f64::floor)
}

fn surprise_microbits_ceil(surprise_bits: f32) -> u64 {
    surprise_microbits(surprise_bits, f64::ceil)
}

fn surprise_microbits(surprise_bits: f32, round: fn(f64) -> f64) -> u64 {
    if !surprise_bits.is_finite() || surprise_bits <= 0.0 {
        return 0;
    }
    round(f64::from(surprise_bits) * 1_000_000.0).min(u64::MAX as f64) as u64
}

impl StorageKernel for FileKernel {
    fn capabilities(&self) -> KernelCapabilities {
        KernelCapabilities::file_append_log()
    }

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
            if persistent_index_contains_cell_id(&self.path, cell.id)?
                || self.index.contains_id(cell.id)
                || !batch_ids.insert(cell.id)
            {
                return Err(KernelError::DuplicateCell);
            }

            cell.system_time = SystemTimeRange::open_from(committed_at);
            cell.commit_id = commit_id;
            stamped.push(cell);
        }

        if stamped.is_empty() {
            return Ok(());
        }

        if persistent_index_contains_commit_id(&self.path, commit_id)?
            || self.index.contains_commit(commit_id)
        {
            return Err(KernelError::DuplicateCommit);
        }

        let appended_canonical_records = stamped.len() + 1;
        let manifest = CommitManifest::new(
            commit_id,
            committed_at,
            stamped.iter().map(|cell| cell.id).collect(),
        );
        let mut encoded = String::new();
        for cell in &stamped {
            encoded.push_str(
                &serde_json::to_string(&FileKernelRecord::Cell {
                    cell: Box::new(cell.clone()),
                    checksum: Some(file_record_checksum(cell)?),
                })
                .map_err(|_error| KernelError::StoreCorrupt)?,
            );
            encoded.push('\n');
        }
        encoded.push_str(
            &serde_json::to_string(&FileKernelRecord::Commit {
                manifest: manifest.clone(),
                checksum: Some(file_record_checksum(&manifest)?),
            })
            .map_err(|_error| KernelError::StoreCorrupt)?,
        );
        encoded.push('\n');
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|_error| KernelError::StoreIo)?;
        write_all_durable(&mut file, encoded.as_bytes())?;

        for cell in stamped {
            self.index.insert(cell)?;
        }
        self.index.apply_explicit_manifest(manifest)?;
        self.health.canonical_records += appended_canonical_records;
        refresh_persistent_index_checkpoint(&self.path)?;
        Ok(())
    }

    fn lookup_cells(&self, lookup: CellLookup) -> Result<Vec<StateCell>, KernelError> {
        if let Some(cell_id) = lookup.cell_id {
            let checkpoint = persistent_index_checkpoint_or_rebuild(&self.path)?;
            let cells = checkpoint
                .cells
                .iter()
                .filter(|address| address.cell_id == cell_id)
                .map(|address| read_cell_at_persistent_address(&self.path, address))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .filter(|cell| FileKernelIndex::cell_matches_lookup(cell, &lookup))
                .collect();
            return Ok(cells);
        }

        if let Some(anchor) = lookup.semantic_anchor.as_ref() {
            let checkpoint = persistent_index_checkpoint_or_rebuild(&self.path)?;
            let cells = checkpoint
                .semantic_anchors
                .iter()
                .filter(|address| address.anchor.as_str() == anchor)
                .map(|address| read_cell_at_persistent_address_parts(&self.path, address))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .filter(|cell| FileKernelIndex::cell_matches_lookup(cell, &lookup))
                .collect();
            return Ok(cells);
        }

        if let Some(commit_id) = lookup.commit_id {
            let checkpoint = persistent_index_checkpoint_or_rebuild(&self.path)?;
            let cells = checkpoint
                .commits
                .iter()
                .filter(|address| address.commit_id == commit_id)
                .map(|address| read_cell_at_persistent_address_parts(&self.path, address))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .filter(|cell| FileKernelIndex::cell_matches_lookup(cell, &lookup))
                .collect();
            return Ok(cells);
        }

        if let Some(scope) = lookup.scope.as_ref() {
            let checkpoint = persistent_index_checkpoint_or_rebuild(&self.path)?;
            let cells = checkpoint
                .scopes
                .iter()
                .filter(|address| &address.scope == scope)
                .map(|address| read_cell_at_persistent_address_parts(&self.path, address))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .filter(|cell| FileKernelIndex::cell_matches_lookup(cell, &lookup))
                .collect();
            return Ok(cells);
        }

        if let Some(activation) = lookup.activation {
            let checkpoint = persistent_index_checkpoint_or_rebuild(&self.path)?;
            let cells = checkpoint
                .activations
                .iter()
                .filter(|address| address.activation == activation)
                .map(|address| read_cell_at_persistent_address_parts(&self.path, address))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .filter(|cell| FileKernelIndex::cell_matches_lookup(cell, &lookup))
                .collect();
            return Ok(cells);
        }

        if let Some(lifecycle_stage) = lookup.lifecycle_stage {
            let checkpoint = persistent_index_checkpoint_or_rebuild(&self.path)?;
            let cells = checkpoint
                .lifecycle_stages
                .iter()
                .filter(|address| address.lifecycle_stage == lifecycle_stage)
                .map(|address| read_cell_at_persistent_address_parts(&self.path, address))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .filter(|cell| FileKernelIndex::cell_matches_lookup(cell, &lookup))
                .collect();
            return Ok(cells);
        }

        if let Some(policy) = lookup.retention_policy {
            let checkpoint = persistent_index_checkpoint_or_rebuild(&self.path)?;
            let cells = checkpoint
                .retention_policies
                .iter()
                .filter(|address| address.policy == policy)
                .map(|address| read_cell_at_persistent_address_parts(&self.path, address))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .filter(|cell| FileKernelIndex::cell_matches_lookup(cell, &lookup))
                .collect();
            return Ok(cells);
        }

        if let Some(policy) = lookup.use_policy {
            let checkpoint = persistent_index_checkpoint_or_rebuild(&self.path)?;
            let cells = checkpoint
                .use_policies
                .iter()
                .filter(|address| address.policy == policy)
                .map(|address| read_cell_at_persistent_address_parts(&self.path, address))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .filter(|cell| FileKernelIndex::cell_matches_lookup(cell, &lookup))
                .collect();
            return Ok(cells);
        }

        if let Some(policy) = lookup.promotion_policy {
            let checkpoint = persistent_index_checkpoint_or_rebuild(&self.path)?;
            let cells = checkpoint
                .promotion_policies
                .iter()
                .filter(|address| address.policy == policy)
                .map(|address| read_cell_at_persistent_address_parts(&self.path, address))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .filter(|cell| FileKernelIndex::cell_matches_lookup(cell, &lookup))
                .collect();
            return Ok(cells);
        }

        if let Some(projection_kind) = lookup.projection_kind {
            let checkpoint = persistent_index_checkpoint_or_rebuild(&self.path)?;
            let cells = checkpoint
                .projection_kinds
                .iter()
                .filter(|address| address.projection_kind == projection_kind)
                .map(|address| read_cell_at_persistent_address_parts(&self.path, address))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .filter(|cell| FileKernelIndex::cell_matches_lookup(cell, &lookup))
                .collect();
            return Ok(cells);
        }

        if let Some(minimum_uncertainty) = lookup.minimum_uncertainty {
            let minimum_microunits = confidence_microunits_floor(minimum_uncertainty);
            let checkpoint = persistent_index_checkpoint_or_rebuild(&self.path)?;
            let cells = checkpoint
                .native_uncertainties
                .iter()
                .filter(|address| address.uncertainty_microunits >= minimum_microunits)
                .map(|address| read_cell_at_persistent_address_parts(&self.path, address))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .filter(|cell| FileKernelIndex::cell_matches_lookup(cell, &lookup))
                .collect();
            return Ok(cells);
        }

        if let Some(minimum_surprise_bits) = lookup.minimum_surprise_bits {
            let minimum_microbits = surprise_microbits_floor(minimum_surprise_bits);
            let checkpoint = persistent_index_checkpoint_or_rebuild(&self.path)?;
            let cells = checkpoint
                .native_surprise_bits
                .iter()
                .filter(|address| address.surprise_microbits >= minimum_microbits)
                .map(|address| read_cell_at_persistent_address_parts(&self.path, address))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .filter(|cell| FileKernelIndex::cell_matches_lookup(cell, &lookup))
                .collect();
            return Ok(cells);
        }

        if let Some(minimum_salience) = lookup.minimum_salience {
            let minimum_microunits = salience_microunits_floor(minimum_salience);
            let checkpoint = persistent_index_checkpoint_or_rebuild(&self.path)?;
            let cells = checkpoint
                .native_saliences
                .iter()
                .filter(|address| address.salience_microunits >= minimum_microunits)
                .map(|address| read_cell_at_persistent_address_parts(&self.path, address))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .filter(|cell| FileKernelIndex::cell_matches_lookup(cell, &lookup))
                .collect();
            return Ok(cells);
        }

        if let Some(minimum_context_affordance) = lookup.minimum_context_affordance {
            let minimum_microunits = salience_microunits_floor(minimum_context_affordance);
            let checkpoint = persistent_index_checkpoint_or_rebuild(&self.path)?;
            let cells = checkpoint
                .native_context_affordances
                .iter()
                .filter(|address| address.context_affordance_microunits >= minimum_microunits)
                .map(|address| read_cell_at_persistent_address_parts(&self.path, address))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .filter(|cell| FileKernelIndex::cell_matches_lookup(cell, &lookup))
                .collect();
            return Ok(cells);
        }

        if let Some(minimum_epistemic_pressure) = lookup.minimum_epistemic_pressure {
            let minimum_microunits = salience_microunits_floor(minimum_epistemic_pressure);
            let checkpoint = persistent_index_checkpoint_or_rebuild(&self.path)?;
            let cells = checkpoint
                .native_epistemic_pressures
                .iter()
                .filter(|address| address.pressure_microunits >= minimum_microunits)
                .map(|address| read_cell_at_persistent_address_parts(&self.path, address))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .filter(|cell| FileKernelIndex::cell_matches_lookup(cell, &lookup))
                .collect();
            return Ok(cells);
        }

        if let Some(strategy) = lookup.trajectory_memory_strategy {
            let checkpoint = persistent_index_checkpoint_or_rebuild(&self.path)?;
            let cells = checkpoint
                .trajectory_memory_strategies
                .iter()
                .filter(|address| address.strategy == strategy)
                .map(|address| read_cell_at_persistent_address_parts(&self.path, address))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .filter(|cell| FileKernelIndex::cell_matches_lookup(cell, &lookup))
                .collect();
            return Ok(cells);
        }

        if let Some(minimum_confidence) = lookup.minimum_trajectory_memory_confidence {
            let minimum_microunits = confidence_microunits_floor(minimum_confidence);
            let checkpoint = persistent_index_checkpoint_or_rebuild(&self.path)?;
            let cells = checkpoint
                .trajectory_memory_confidences
                .iter()
                .filter(|address| address.confidence_microunits >= minimum_microunits)
                .map(|address| read_cell_at_persistent_address_parts(&self.path, address))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .filter(|cell| FileKernelIndex::cell_matches_lookup(cell, &lookup))
                .collect();
            return Ok(cells);
        }

        if let Some(kind) = lookup.context_gap_kind {
            let checkpoint = persistent_index_checkpoint_or_rebuild(&self.path)?;
            let cells = checkpoint
                .context_gap_kinds
                .iter()
                .filter(|address| address.kind == kind)
                .map(|address| read_cell_at_persistent_address_parts(&self.path, address))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .filter(|cell| FileKernelIndex::cell_matches_lookup(cell, &lookup))
                .collect();
            return Ok(cells);
        }

        if let Some(minimum_context_gap_priority) = lookup.minimum_context_gap_priority {
            let minimum_microunits = confidence_microunits_floor(minimum_context_gap_priority);
            let checkpoint = persistent_index_checkpoint_or_rebuild(&self.path)?;
            let cells = checkpoint
                .context_gap_priorities
                .iter()
                .filter(|address| address.priority_microunits >= minimum_microunits)
                .map(|address| read_cell_at_persistent_address_parts(&self.path, address))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .filter(|cell| FileKernelIndex::cell_matches_lookup(cell, &lookup))
                .collect();
            return Ok(cells);
        }

        if let Some(kind) = lookup.invalidation_condition_kind {
            let checkpoint = persistent_index_checkpoint_or_rebuild(&self.path)?;
            let cells = checkpoint
                .invalidation_condition_kinds
                .iter()
                .filter(|address| address.kind == kind)
                .map(|address| read_cell_at_persistent_address_parts(&self.path, address))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .filter(|cell| FileKernelIndex::cell_matches_lookup(cell, &lookup))
                .collect();
            return Ok(cells);
        }

        if let Some(minimum_invalidation_priority) = lookup.minimum_invalidation_priority {
            let minimum_microunits = confidence_microunits_floor(minimum_invalidation_priority);
            let checkpoint = persistent_index_checkpoint_or_rebuild(&self.path)?;
            let cells = checkpoint
                .invalidation_condition_priorities
                .iter()
                .filter(|address| address.priority_microunits >= minimum_microunits)
                .map(|address| read_cell_at_persistent_address_parts(&self.path, address))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .filter(|cell| FileKernelIndex::cell_matches_lookup(cell, &lookup))
                .collect();
            return Ok(cells);
        }

        if let Some(action) = lookup.epistemic_action {
            let checkpoint = persistent_index_checkpoint_or_rebuild(&self.path)?;
            let cells = checkpoint
                .epistemic_actions
                .iter()
                .filter(|address| address.action == action)
                .map(|address| read_cell_at_persistent_address_parts(&self.path, address))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .filter(|cell| FileKernelIndex::cell_matches_lookup(cell, &lookup))
                .collect();
            return Ok(cells);
        }

        if let Some(reason) = lookup.epistemic_action_reason {
            let checkpoint = persistent_index_checkpoint_or_rebuild(&self.path)?;
            let cells = checkpoint
                .epistemic_action_reasons
                .iter()
                .filter(|address| address.reason == reason)
                .map(|address| read_cell_at_persistent_address_parts(&self.path, address))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .filter(|cell| FileKernelIndex::cell_matches_lookup(cell, &lookup))
                .collect();
            return Ok(cells);
        }

        if let Some(question) = lookup.answerability_question.as_ref() {
            let checkpoint = persistent_index_checkpoint_or_rebuild(&self.path)?;
            let cells = checkpoint
                .answerability_questions
                .iter()
                .filter(|address| &address.question == question)
                .map(|address| read_cell_at_persistent_address_parts(&self.path, address))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .filter(|cell| FileKernelIndex::cell_matches_lookup(cell, &lookup))
                .collect();
            return Ok(cells);
        }

        if let Some(source) = lookup.evidence_source.as_ref() {
            let checkpoint = persistent_index_checkpoint_or_rebuild(&self.path)?;
            let cells = checkpoint
                .evidence_sources
                .iter()
                .filter(|address| &address.source == source)
                .map(|address| read_cell_at_persistent_address_parts(&self.path, address))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .filter(|cell| FileKernelIndex::cell_matches_lookup(cell, &lookup))
                .collect();
            return Ok(cells);
        }

        if let Some(minimum_confidence) = lookup.minimum_confidence {
            let minimum_microunits = confidence_microunits_floor(minimum_confidence);
            let checkpoint = persistent_index_checkpoint_or_rebuild(&self.path)?;
            let cells = checkpoint
                .max_evidence_confidences
                .iter()
                .filter(|address| address.confidence_microunits >= minimum_microunits)
                .map(|address| read_cell_at_persistent_address_parts(&self.path, address))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .filter(|cell| FileKernelIndex::cell_matches_lookup(cell, &lookup))
                .collect();
            return Ok(cells);
        }

        if let Some(system_at) = lookup.system_at {
            let checkpoint = persistent_index_checkpoint_or_rebuild(&self.path)?;
            let cells = checkpoint
                .system_times
                .iter()
                .filter(|address| address.system_from <= system_at)
                .map(|address| read_cell_at_persistent_address_parts(&self.path, address))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .filter(|cell| FileKernelIndex::cell_matches_lookup(cell, &lookup))
                .collect();
            return Ok(cells);
        }

        if let Some(valid_at) = lookup.valid_at {
            let checkpoint = persistent_index_checkpoint_or_rebuild(&self.path)?;
            let cells = checkpoint
                .valid_times
                .iter()
                .filter(|address| address.valid_from <= valid_at)
                .map(|address| read_cell_at_persistent_address_parts(&self.path, address))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .filter(|cell| FileKernelIndex::cell_matches_lookup(cell, &lookup))
                .collect();
            return Ok(cells);
        }

        if let Some(target) = lookup.dependency_target {
            let checkpoint = persistent_index_checkpoint_or_rebuild(&self.path)?;
            let cells = checkpoint
                .dependency_targets
                .iter()
                .filter(|address| address.target == target)
                .map(|address| read_cell_at_persistent_address_parts(&self.path, address))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .filter(|cell| FileKernelIndex::cell_matches_lookup(cell, &lookup))
                .collect();
            return Ok(cells);
        }

        if let Some(kind) = lookup.dependency_kind {
            let checkpoint = persistent_index_checkpoint_or_rebuild(&self.path)?;
            let cells = checkpoint
                .dependency_kinds
                .iter()
                .filter(|address| address.kind == kind)
                .map(|address| read_cell_at_persistent_address_parts(&self.path, address))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .filter(|cell| FileKernelIndex::cell_matches_lookup(cell, &lookup))
                .collect();
            return Ok(cells);
        }

        let checkpoint = persistent_index_checkpoint_or_rebuild(&self.path)?;
        if !checkpoint.cells.is_empty() {
            let cells = checkpoint
                .cells
                .iter()
                .map(|address| read_cell_at_persistent_address_parts(&self.path, address))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .filter(|cell| FileKernelIndex::cell_matches_lookup(cell, &lookup))
                .collect();
            return Ok(cells);
        }

        let candidates = self
            .index
            .candidate_positions(&lookup)
            .into_iter()
            .map(|position| &self.index.cells[position])
            .collect::<Vec<_>>();

        let cells = candidates
            .into_iter()
            .filter(|cell| FileKernelIndex::cell_matches_lookup(cell, &lookup))
            .cloned()
            .collect();

        Ok(cells)
    }

    fn lookup_commit_manifest(
        &self,
        commit_id: CommitId,
    ) -> Result<Option<CommitManifest>, KernelError> {
        let checkpoint = persistent_index_checkpoint_or_rebuild(&self.path)?;
        checkpoint
            .commit_manifests
            .iter()
            .find(|address| address.commit_id == commit_id)
            .map(|address| read_commit_manifest_at_persistent_address_parts(&self.path, address))
            .transpose()
            .map(|manifest| manifest.or_else(|| self.index.manifests.get(&commit_id).cloned()))
    }

    fn list_commit_manifests(&self) -> Result<Vec<CommitManifest>, KernelError> {
        let checkpoint = persistent_index_checkpoint_or_rebuild(&self.path)?;
        if checkpoint.commit_manifests.is_empty() {
            return Ok(self.index.list_manifests());
        }

        checkpoint
            .commit_manifests
            .iter()
            .map(|address| read_commit_manifest_at_persistent_address_parts(&self.path, address))
            .collect()
    }

    fn list_commit_manifests_matching(
        &self,
        lookup: CommitManifestLookup,
    ) -> Result<Vec<CommitManifest>, KernelError> {
        let checkpoint = persistent_index_checkpoint_or_rebuild(&self.path)?;
        if checkpoint.commit_manifests.is_empty() {
            return self.index.list_manifests_matching(lookup);
        }

        let start = if let Some(after) = lookup.after {
            checkpoint
                .commit_manifests
                .iter()
                .position(|address| address.commit_id == after)
                .map(|position| position + 1)
                .ok_or(KernelError::CommitNotFound)?
        } else {
            0
        };
        let limit = lookup.limit.unwrap_or(usize::MAX);

        checkpoint
            .commit_manifests
            .iter()
            .skip(start)
            .take(limit)
            .map(|address| read_commit_manifest_at_persistent_address_parts(&self.path, address))
            .collect()
    }

    fn append_revision_link(
        &mut self,
        revision_link: RevisionLinkRecord,
    ) -> Result<(), KernelError> {
        if persistent_index_contains_revision_link(&self.path, &revision_link)?
            || self.index.revision_links.contains(&revision_link)
        {
            return Err(KernelError::DuplicateRevisionLink);
        }

        let encoded = serde_json::to_string(&FileKernelRecord::RevisionLink {
            revision_link: revision_link.clone(),
            checksum: Some(file_record_checksum(&revision_link)?),
        })
        .map_err(|_error| KernelError::StoreCorrupt)?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|_error| KernelError::StoreIo)?;
        write_all_durable(&mut file, format!("{encoded}\n").as_bytes())?;

        self.index.insert_revision_link(revision_link)?;
        self.health.canonical_records += 1;
        refresh_persistent_index_checkpoint(&self.path)?;
        Ok(())
    }

    fn list_revision_links(
        &self,
        lookup: RevisionLinkLookup,
    ) -> Result<Vec<RevisionLinkRecord>, KernelError> {
        if let Some(source) = lookup.source {
            let checkpoint = persistent_index_checkpoint_or_rebuild(&self.path)?;
            let revision_links = checkpoint
                .revision_link_sources
                .iter()
                .filter(|address| address.source == source)
                .map(|address| read_revision_link_at_persistent_address_parts(&self.path, address))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .filter(|revision_link| {
                    lookup
                        .target
                        .map_or(true, |target| revision_link.target == target)
                })
                .filter(|revision_link| lookup.kind.map_or(true, |kind| revision_link.kind == kind))
                .collect();
            return Ok(revision_links);
        }

        if let Some(target) = lookup.target {
            let checkpoint = persistent_index_checkpoint_or_rebuild(&self.path)?;
            let revision_links = checkpoint
                .revision_link_targets
                .iter()
                .filter(|address| address.target == target)
                .map(|address| read_revision_link_at_persistent_address_parts(&self.path, address))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .filter(|revision_link| lookup.kind.map_or(true, |kind| revision_link.kind == kind))
                .collect();
            return Ok(revision_links);
        }

        if let Some(kind) = lookup.kind {
            let checkpoint = persistent_index_checkpoint_or_rebuild(&self.path)?;
            let revision_links = checkpoint
                .revision_link_kinds
                .iter()
                .filter(|address| address.kind == kind)
                .map(|address| read_revision_link_at_persistent_address_parts(&self.path, address))
                .collect::<Result<Vec<_>, _>>()?;
            return Ok(revision_links);
        }

        let checkpoint = persistent_index_checkpoint_or_rebuild(&self.path)?;
        checkpoint
            .revision_link_sources
            .iter()
            .map(|address| read_revision_link_at_persistent_address_parts(&self.path, address))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        sync_parent_directory, write_all_durable, CellLookup, CommitManifestLookup, FileKernel,
        FileKernelIndexedConstraintPlan, KernelCapabilities, KernelDurability, KernelError,
        KernelRequirements, RevisionLinkLookup, StorageKernel,
    };
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
    use std::{
        fs,
        io::{Read, Seek, SeekFrom},
        path::{Path, PathBuf},
    };

    fn temp_kernel_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("{name}-{:?}.jsonl", StateCellId::new()))
    }

    fn temp_persistent_index_path(path: &Path) -> PathBuf {
        let file_name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "continuitydb.jsonl".to_string());
        path.with_file_name(format!("{file_name}.index.json"))
    }

    fn test_commit_time() -> Result<chrono::DateTime<Utc>, Box<dyn std::error::Error>> {
        Utc.with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp").into())
    }

    fn append_committed(
        kernel: &mut FileKernel,
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
    fn file_kernel_writes_versioned_primary_persistent_index(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-primary-persistent-index");
        let index_path = temp_persistent_index_path(&path);
        let committed_at = test_commit_time()?;
        let commit_id = CommitId::new();
        let mut cell = sample_cell("project:continuitydb:persistent-primary-index", 0.91, 12)?;
        cell.set_context_affordance(ContextAffordance::new(0.95, 0.9, 0.8, 0.4, 0.95, 0.1)?);
        let expected_cell_id = serde_json::to_value(cell.id)?;
        let mut kernel = FileKernel::open(&path)?;
        kernel.append_cell_at_with_commit_id(cell, committed_at, commit_id)?;

        let checkpoint: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&index_path)?)?;
        assert_eq!("continuitydb.file_kernel.index", checkpoint["format"]);
        assert_eq!(12, checkpoint["version"]);
        assert_eq!(1, checkpoint["cell_record_count"]);
        assert_eq!(1, checkpoint["commit_manifest_record_count"]);
        assert_eq!(0, checkpoint["revision_link_record_count"]);
        assert_eq!(1, checkpoint["semantic_anchor_address_count"]);
        assert_eq!(1, checkpoint["commit_address_count"]);
        assert_eq!(1, checkpoint["scope_address_count"]);
        assert_eq!(1, checkpoint["activation_address_count"]);
        assert_eq!(1, checkpoint["lifecycle_stage_address_count"]);
        assert_eq!(1, checkpoint["retention_policy_address_count"]);
        assert_eq!(1, checkpoint["use_policy_address_count"]);
        assert_eq!(1, checkpoint["promotion_policy_address_count"]);
        assert_eq!(0, checkpoint["projection_kind_address_count"]);
        assert_eq!(1, checkpoint["native_uncertainty_address_count"]);
        assert_eq!(1, checkpoint["native_surprise_bits_address_count"]);
        assert_eq!(1, checkpoint["native_salience_address_count"]);
        assert_eq!(1, checkpoint["native_context_affordance_address_count"]);
        assert_eq!(1, checkpoint["native_epistemic_pressure_address_count"]);
        assert_eq!(0, checkpoint["trajectory_memory_strategy_address_count"]);
        assert_eq!(0, checkpoint["trajectory_memory_confidence_address_count"]);
        assert_eq!(0, checkpoint["context_gap_kind_address_count"]);
        assert_eq!(0, checkpoint["context_gap_priority_address_count"]);
        assert_eq!(0, checkpoint["invalidation_condition_kind_address_count"]);
        assert_eq!(
            0,
            checkpoint["invalidation_condition_priority_address_count"]
        );
        assert_eq!(1, checkpoint["epistemic_action_address_count"]);
        assert_eq!(0, checkpoint["epistemic_action_reason_address_count"]);
        assert_eq!(1, checkpoint["answerability_question_address_count"]);
        assert_eq!(1, checkpoint["evidence_source_address_count"]);
        assert_eq!(1, checkpoint["max_evidence_confidence_address_count"]);
        assert_eq!(1, checkpoint["system_time_address_count"]);
        assert_eq!(1, checkpoint["valid_time_address_count"]);
        assert_eq!(0, checkpoint["dependency_target_address_count"]);
        assert_eq!(0, checkpoint["dependency_kind_address_count"]);
        let cells = checkpoint["cells"]
            .as_array()
            .ok_or_else(|| std::io::Error::other("missing persistent cells"))?;
        assert_eq!(1, cells.len());
        assert_eq!(expected_cell_id, cells[0]["cell_id"]);
        let offset = cells[0]["offset"]
            .as_u64()
            .ok_or_else(|| std::io::Error::other("missing cell offset"))?;
        let length = cells[0]["length"]
            .as_u64()
            .ok_or_else(|| std::io::Error::other("missing cell length"))?;
        let checksum = cells[0]["checksum"]
            .as_str()
            .ok_or_else(|| std::io::Error::other("missing cell checksum"))?;
        assert!(offset > 0);
        assert!(length > 0);
        assert!(checksum.starts_with("continuitydb-fnv1a64:"));
        let epistemic_actions = checkpoint["epistemic_actions"]
            .as_array()
            .ok_or_else(|| std::io::Error::other("missing epistemic action addresses"))?;
        assert_eq!(1, epistemic_actions.len());
        assert_eq!("Use", epistemic_actions[0]["action"]);
        assert_eq!(expected_cell_id, epistemic_actions[0]["cell_id"]);
        let native_epistemic_pressures = checkpoint["native_epistemic_pressures"]
            .as_array()
            .ok_or_else(|| std::io::Error::other("missing epistemic pressure addresses"))?;
        assert_eq!(1, native_epistemic_pressures.len());
        assert_eq!(expected_cell_id, native_epistemic_pressures[0]["cell_id"]);
        let native_context_affordances = checkpoint["native_context_affordances"]
            .as_array()
            .ok_or_else(|| std::io::Error::other("missing context affordance addresses"))?;
        assert_eq!(1, native_context_affordances.len());
        assert_eq!(
            720_000,
            native_context_affordances[0]["context_affordance_microunits"]
        );
        assert_eq!(expected_cell_id, native_context_affordances[0]["cell_id"]);
        let retention_policies = checkpoint["retention_policies"]
            .as_array()
            .ok_or_else(|| std::io::Error::other("missing retention policy addresses"))?;
        assert_eq!(1, retention_policies.len());
        assert_eq!("Persistent", retention_policies[0]["policy"]);
        assert_eq!(expected_cell_id, retention_policies[0]["cell_id"]);
        let use_policies = checkpoint["use_policies"]
            .as_array()
            .ok_or_else(|| std::io::Error::other("missing use policy addresses"))?;
        assert_eq!(1, use_policies.len());
        assert_eq!("UseDirectly", use_policies[0]["policy"]);
        assert_eq!(expected_cell_id, use_policies[0]["cell_id"]);
        let promotion_policies = checkpoint["promotion_policies"]
            .as_array()
            .ok_or_else(|| std::io::Error::other("missing promotion policy addresses"))?;
        assert_eq!(1, promotion_policies.len());
        assert_eq!("Manual", promotion_policies[0]["policy"]);
        assert_eq!(expected_cell_id, promotion_policies[0]["cell_id"]);
        let epistemic_action_reasons = checkpoint["epistemic_action_reasons"]
            .as_array()
            .ok_or_else(|| std::io::Error::other("missing epistemic action reason addresses"))?;
        assert!(epistemic_action_reasons.is_empty());

        let mut record_bytes = vec![0; length as usize];
        let mut log_file = std::fs::File::open(&path)?;
        log_file.seek(SeekFrom::Start(offset))?;
        log_file.read_exact(&mut record_bytes)?;
        let record: serde_json::Value = serde_json::from_slice(&record_bytes)?;
        assert_eq!("cell", record["type"]);
        assert_eq!(expected_cell_id, record["cell"]["id"]);

        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_cell_id_lookup_uses_primary_persistent_index_address(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-primary-index-lookup");
        let index_path = temp_persistent_index_path(&path);
        let expected = {
            let mut kernel = FileKernel::open(&path)?;
            let cell = sample_cell("project:continuitydb:persistent-cell-id-lookup", 0.91, 12)?;
            append_committed(&mut kernel, cell)?
        };
        let mut reopened = FileKernel::open(&path)?;
        reopened.index.ids.clear();
        reopened.index.cells.clear();

        let cells = reopened.lookup_cells(CellLookup {
            cell_id: Some(expected.id),
            ..CellLookup::default()
        })?;

        assert_eq!(vec![expected], cells);

        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_semantic_anchor_lookup_uses_persistent_index_addresses(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-semantic-anchor-index-lookup");
        let index_path = temp_persistent_index_path(&path);
        let expected = {
            let mut kernel = FileKernel::open(&path)?;
            let matching = sample_cell("project:continuitydb:persistent-anchor-lookup", 0.91, 12)?;
            let unrelated =
                sample_cell("project:continuitydb:persistent-anchor-unrelated", 0.83, 15)?;
            let expected = append_committed(&mut kernel, matching)?;
            append_committed(&mut kernel, unrelated)?;
            expected
        };
        let mut reopened = FileKernel::open(&path)?;
        reopened.index.anchors.clear();
        reopened.index.cells.clear();

        let cells = reopened.lookup_cells(CellLookup {
            semantic_anchor: Some("project:continuitydb:persistent-anchor-lookup".to_string()),
            ..CellLookup::default()
        })?;

        assert_eq!(vec![expected], cells);

        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_commit_lookup_uses_persistent_index_addresses(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-commit-index-lookup");
        let index_path = temp_persistent_index_path(&path);
        let committed_at = test_commit_time()?;
        let matching_commit = CommitId::new();
        let unrelated_commit = CommitId::new();
        let first = sample_cell("project:continuitydb:persistent-commit-first", 0.91, 12)?;
        let second = sample_cell("project:continuitydb:persistent-commit-second", 0.88, 13)?;
        let unrelated = sample_cell("project:continuitydb:persistent-commit-unrelated", 0.72, 15)?;
        let expected = {
            let mut kernel = FileKernel::open(&path)?;
            kernel.append_cells_at_with_commit_id(
                vec![first.clone(), second.clone()],
                committed_at,
                matching_commit,
            )?;
            kernel.append_cell_at_with_commit_id(unrelated, committed_at, unrelated_commit)?;
            vec![
                StateCell {
                    system_time: continuitydb_core::SystemTimeRange::open_from(committed_at),
                    commit_id: matching_commit,
                    ..first
                },
                StateCell {
                    system_time: continuitydb_core::SystemTimeRange::open_from(committed_at),
                    commit_id: matching_commit,
                    ..second
                },
            ]
        };
        let mut reopened = FileKernel::open(&path)?;
        reopened.index.commits.clear();
        reopened.index.cells.clear();

        let cells = reopened.lookup_cells(CellLookup {
            commit_id: Some(matching_commit),
            ..CellLookup::default()
        })?;

        assert_eq!(expected, cells);

        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rebuilds_tampered_commit_persistent_index_key(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-tampered-commit-index-key");
        let index_path = temp_persistent_index_path(&path);
        let committed_at = test_commit_time()?;
        let commit_id = CommitId::new();
        let cell = sample_cell("project:continuitydb:tampered-commit-index-key", 0.91, 12)?;
        let expected_id = cell.id;
        let mut kernel = FileKernel::open(&path)?;
        kernel.append_cell_at_with_commit_id(cell, committed_at, commit_id)?;

        let mut checkpoint: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&index_path)?)?;
        checkpoint["commits"][0]["commit_id"] = serde_json::to_value(CommitId::new())?;
        fs::write(&index_path, serde_json::to_vec_pretty(&checkpoint)?)?;
        kernel.index.commits.clear();
        kernel.index.cells.clear();

        let results = kernel.lookup_cells(CellLookup {
            commit_id: Some(commit_id),
            ..CellLookup::default()
        })?;

        assert_eq!(
            results.iter().map(|cell| cell.id).collect::<Vec<_>>(),
            vec![expected_id]
        );
        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_scope_lookup_uses_persistent_index_addresses(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-scope-index-lookup");
        let index_path = temp_persistent_index_path(&path);
        let expected = {
            let mut kernel = FileKernel::open(&path)?;
            let project = StateCell {
                scope: Scope::Project("continuitydb".to_string()),
                ..sample_cell("project:continuitydb:persistent-scope-lookup", 0.91, 12)?
            };
            let team = StateCell {
                scope: Scope::Team("storage".to_string()),
                ..sample_cell("project:continuitydb:persistent-scope-unrelated", 0.83, 15)?
            };
            let expected = append_committed(&mut kernel, project)?;
            append_committed(&mut kernel, team)?;
            expected
        };
        let mut reopened = FileKernel::open(&path)?;
        reopened.index.scopes.clear();
        reopened.index.cells.clear();

        let cells = reopened.lookup_cells(CellLookup {
            scope: Some(Scope::Project("continuitydb".to_string())),
            ..CellLookup::default()
        })?;

        assert_eq!(vec![expected], cells);

        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rebuilds_tampered_scope_persistent_index_key(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-tampered-scope-index-key");
        let index_path = temp_persistent_index_path(&path);
        let committed_at = test_commit_time()?;
        let scope = Scope::Project("continuitydb".to_string());
        let cell = StateCell {
            scope: scope.clone(),
            ..sample_cell("project:continuitydb:tampered-scope-index-key", 0.91, 12)?
        };
        let expected_id = cell.id;
        let mut kernel = FileKernel::open(&path)?;
        kernel.append_cells_at(vec![cell], committed_at)?;

        let mut checkpoint: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&index_path)?)?;
        checkpoint["scopes"][0]["scope"] =
            serde_json::to_value(Scope::Team("storage".to_string()))?;
        fs::write(&index_path, serde_json::to_vec_pretty(&checkpoint)?)?;
        kernel.index.scopes.clear();
        kernel.index.cells.clear();

        let results = kernel.lookup_cells(CellLookup {
            scope: Some(scope),
            ..CellLookup::default()
        })?;

        assert_eq!(
            results.iter().map(|cell| cell.id).collect::<Vec<_>>(),
            vec![expected_id]
        );
        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_activation_lookup_uses_persistent_index_addresses(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-activation-index-lookup");
        let index_path = temp_persistent_index_path(&path);
        let expected = {
            let mut kernel = FileKernel::open(&path)?;
            let active = StateCell {
                activation: ActivationState::Active,
                ..sample_cell(
                    "project:continuitydb:persistent-activation-unrelated",
                    0.83,
                    15,
                )?
            };
            let frontier = StateCell {
                activation: ActivationState::Frontier,
                ..sample_cell(
                    "project:continuitydb:persistent-activation-lookup",
                    0.91,
                    12,
                )?
            };
            append_committed(&mut kernel, active)?;
            append_committed(&mut kernel, frontier)?
        };
        let mut reopened = FileKernel::open(&path)?;
        reopened.index.activations.clear();
        reopened.index.cells.clear();

        let cells = reopened.lookup_cells(CellLookup {
            activation: Some(ActivationState::Frontier),
            ..CellLookup::default()
        })?;

        assert_eq!(vec![expected], cells);

        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rebuilds_tampered_activation_persistent_index_key(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-tampered-activation-index-key");
        let index_path = temp_persistent_index_path(&path);
        let committed_at = test_commit_time()?;
        let cell = StateCell {
            activation: ActivationState::Frontier,
            ..sample_cell(
                "project:continuitydb:tampered-activation-index-key",
                0.91,
                12,
            )?
        };
        let expected_id = cell.id;
        let mut kernel = FileKernel::open(&path)?;
        kernel.append_cells_at(vec![cell], committed_at)?;

        let mut checkpoint: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&index_path)?)?;
        checkpoint["activations"][0]["activation"] = serde_json::to_value(ActivationState::Active)?;
        fs::write(&index_path, serde_json::to_vec_pretty(&checkpoint)?)?;
        kernel.index.activations.clear();
        kernel.index.cells.clear();

        let results = kernel.lookup_cells(CellLookup {
            activation: Some(ActivationState::Frontier),
            ..CellLookup::default()
        })?;

        assert_eq!(
            results.iter().map(|cell| cell.id).collect::<Vec<_>>(),
            vec![expected_id]
        );
        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_lifecycle_stage_lookup_uses_persistent_index_addresses(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-lifecycle-stage-index-lookup");
        let index_path = temp_persistent_index_path(&path);
        let expected = {
            let mut kernel = FileKernel::open(&path)?;
            let observed = sample_cell(
                "project:continuitydb:persistent-lifecycle-unrelated",
                0.83,
                15,
            )?;
            let operationalized =
                sample_cell("project:continuitydb:persistent-lifecycle-lookup", 0.91, 12)?
                    .with_lifecycle_stage(LifecycleStage::Operationalized);
            append_committed(&mut kernel, observed)?;
            append_committed(&mut kernel, operationalized)?
        };
        let mut reopened = FileKernel::open(&path)?;
        reopened.index.lifecycle_stages.clear();
        reopened.index.cells.clear();

        let cells = reopened.lookup_cells(CellLookup {
            lifecycle_stage: Some(LifecycleStage::Operationalized),
            ..CellLookup::default()
        })?;

        assert_eq!(vec![expected], cells);

        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_lookup_plan_indexes_lifecycle_stage() -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-lifecycle-lookup-plan");
        let mut kernel = FileKernel::open(&path)?;
        append_committed(
            &mut kernel,
            sample_cell("project:continuitydb:lifecycle-plan-observed", 0.83, 15)?,
        )?;
        append_committed(
            &mut kernel,
            sample_cell(
                "project:continuitydb:lifecycle-plan-operationalized",
                0.91,
                12,
            )?
            .with_lifecycle_stage(LifecycleStage::Operationalized),
        )?;

        let plan = kernel.lookup_plan(&CellLookup {
            lifecycle_stage: Some(LifecycleStage::Operationalized),
            ..CellLookup::default()
        });

        assert_eq!(plan.indexed_constraints, vec!["lifecycle_stage"]);
        assert_eq!(plan.residual_exact_constraints, Vec::<&'static str>::new());
        assert_eq!(plan.candidate_count, 1);
        assert!(!plan.full_scan);

        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rebuilds_tampered_lifecycle_stage_persistent_index_key(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-tampered-lifecycle-index-key");
        let index_path = temp_persistent_index_path(&path);
        let committed_at = test_commit_time()?;
        let cell = sample_cell(
            "project:continuitydb:tampered-lifecycle-index-key",
            0.91,
            12,
        )?
        .with_lifecycle_stage(LifecycleStage::Operationalized);
        let expected_id = cell.id;
        let mut kernel = FileKernel::open(&path)?;
        kernel.append_cells_at(vec![cell], committed_at)?;

        let mut checkpoint: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&index_path)?)?;
        checkpoint["lifecycle_stages"][0]["lifecycle_stage"] =
            serde_json::to_value(LifecycleStage::Observed)?;
        fs::write(&index_path, serde_json::to_vec_pretty(&checkpoint)?)?;
        kernel.index.lifecycle_stages.clear();
        kernel.index.cells.clear();

        let results = kernel.lookup_cells(CellLookup {
            lifecycle_stage: Some(LifecycleStage::Operationalized),
            ..CellLookup::default()
        })?;

        assert_eq!(
            results.iter().map(|cell| cell.id).collect::<Vec<_>>(),
            vec![expected_id]
        );
        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_projection_kind_lookup_uses_persistent_index_addresses(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-projection-kind-index-lookup");
        let index_path = temp_persistent_index_path(&path);
        let expected = {
            let mut kernel = FileKernel::open(&path)?;
            let observed = sample_cell(
                "project:continuitydb:persistent-projection-unrelated",
                0.83,
                15,
            )?;
            let mut procedural = sample_cell(
                "project:continuitydb:persistent-projection-lookup",
                0.91,
                12,
            )?;
            procedural.add_projection(MemoryProjection::new(
                MemoryProjectionKind::Procedural,
                "Run the validated checkout lifecycle before answering.",
                Confidence::new(0.88)?,
                CellCost::new(11, 0)?,
            )?);
            append_committed(&mut kernel, observed)?;
            append_committed(&mut kernel, procedural)?
        };
        let mut reopened = FileKernel::open(&path)?;
        reopened.index.projection_kinds.clear();
        reopened.index.cells.clear();

        let cells = reopened.lookup_cells(CellLookup {
            projection_kind: Some(MemoryProjectionKind::Procedural),
            ..CellLookup::default()
        })?;

        assert_eq!(vec![expected], cells);

        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_lookup_plan_indexes_projection_kind() -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-projection-lookup-plan");
        let mut kernel = FileKernel::open(&path)?;
        append_committed(
            &mut kernel,
            sample_cell("project:continuitydb:projection-plan-observed", 0.83, 15)?,
        )?;
        let mut policy = sample_cell("project:continuitydb:projection-plan-policy", 0.91, 12)?;
        policy.add_projection(MemoryProjection::new(
            MemoryProjectionKind::Policy,
            "Do not collapse unresolved evidence into asserted truth.",
            Confidence::new(0.9)?,
            CellCost::new(9, 0)?,
        )?);
        append_committed(&mut kernel, policy)?;

        let plan = kernel.lookup_plan(&CellLookup {
            projection_kind: Some(MemoryProjectionKind::Policy),
            ..CellLookup::default()
        });

        assert_eq!(plan.indexed_constraints, vec!["projection_kind"]);
        assert_eq!(plan.residual_exact_constraints, Vec::<&'static str>::new());
        assert_eq!(plan.candidate_count, 1);
        assert!(!plan.full_scan);

        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rebuilds_tampered_projection_kind_persistent_index_key(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-tampered-projection-index-key");
        let index_path = temp_persistent_index_path(&path);
        let committed_at = test_commit_time()?;
        let mut cell = sample_cell(
            "project:continuitydb:tampered-projection-index-key",
            0.91,
            12,
        )?;
        cell.add_projection(MemoryProjection::new(
            MemoryProjectionKind::Uncertainty,
            "Baseline failed; verify before operational use.",
            Confidence::new(0.83)?,
            CellCost::new(8, 0)?,
        )?);
        let expected_id = cell.id;
        let mut kernel = FileKernel::open(&path)?;
        kernel.append_cells_at(vec![cell], committed_at)?;

        let mut checkpoint: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&index_path)?)?;
        checkpoint["projection_kinds"][0]["projection_kind"] =
            serde_json::to_value(MemoryProjectionKind::Semantic)?;
        fs::write(&index_path, serde_json::to_vec_pretty(&checkpoint)?)?;
        kernel.index.projection_kinds.clear();
        kernel.index.cells.clear();

        let results = kernel.lookup_cells(CellLookup {
            projection_kind: Some(MemoryProjectionKind::Uncertainty),
            ..CellLookup::default()
        })?;

        assert_eq!(
            results.iter().map(|cell| cell.id).collect::<Vec<_>>(),
            vec![expected_id]
        );
        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_uncertainty_threshold_lookup_uses_persistent_index_addresses(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-uncertainty-index-lookup");
        let index_path = temp_persistent_index_path(&path);
        let expected = {
            let mut kernel = FileKernel::open(&path)?;
            let mut routine = sample_cell(
                "project:continuitydb:persistent-uncertainty-unrelated",
                0.83,
                15,
            )?;
            routine.set_uncertainty(EpistemicUncertainty::new(
                Confidence::new(0.2)?,
                0.5,
                "routine ambiguity",
            )?);
            let mut uncertain = sample_cell(
                "project:continuitydb:persistent-uncertainty-lookup",
                0.91,
                12,
            )?;
            uncertain.set_uncertainty(EpistemicUncertainty::new(
                Confidence::new(0.82)?,
                4.25,
                "baseline belief failed",
            )?);
            append_committed(&mut kernel, routine)?;
            append_committed(&mut kernel, uncertain)?
        };
        let mut reopened = FileKernel::open(&path)?;
        reopened.index.native_uncertainties.clear();
        reopened.index.cells.clear();

        let cells = reopened.lookup_cells(CellLookup {
            minimum_uncertainty: Some(Confidence::new(0.7)?),
            ..CellLookup::default()
        })?;

        assert_eq!(vec![expected], cells);

        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_surprise_threshold_lookup_uses_persistent_index_addresses(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-surprise-index-lookup");
        let index_path = temp_persistent_index_path(&path);
        let expected = {
            let mut kernel = FileKernel::open(&path)?;
            let mut routine = sample_cell(
                "project:continuitydb:persistent-surprise-unrelated",
                0.83,
                15,
            )?;
            routine.set_uncertainty(EpistemicUncertainty::new(
                Confidence::new(0.6)?,
                0.75,
                "expected variance",
            )?);
            let mut surprising =
                sample_cell("project:continuitydb:persistent-surprise-lookup", 0.91, 12)?;
            surprising.set_uncertainty(EpistemicUncertainty::new(
                Confidence::new(0.64)?,
                6.5,
                "high certainty baseline failed",
            )?);
            append_committed(&mut kernel, routine)?;
            append_committed(&mut kernel, surprising)?
        };
        let mut reopened = FileKernel::open(&path)?;
        reopened.index.native_surprise_bits.clear();
        reopened.index.cells.clear();

        let cells = reopened.lookup_cells(CellLookup {
            minimum_surprise_bits: Some(3.0),
            ..CellLookup::default()
        })?;

        assert_eq!(vec![expected], cells);

        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_salience_threshold_lookup_uses_persistent_index_addresses(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-salience-index-lookup");
        let index_path = temp_persistent_index_path(&path);
        let expected = {
            let mut kernel = FileKernel::open(&path)?;
            let mut routine = sample_cell(
                "project:continuitydb:persistent-salience-unrelated",
                0.83,
                15,
            )?;
            routine.set_attention(AttentionSignal::new(0.1, 0.1, 0.2, 0.0)?);
            let mut salient =
                sample_cell("project:continuitydb:persistent-salience-lookup", 0.91, 12)?;
            salient.set_attention(AttentionSignal::new(0.9, 0.8, 0.9, 0.8)?);
            append_committed(&mut kernel, routine)?;
            append_committed(&mut kernel, salient)?
        };
        let mut reopened = FileKernel::open(&path)?;
        reopened.index.native_saliences.clear();
        reopened.index.cells.clear();

        let cells = reopened.lookup_cells(CellLookup {
            minimum_salience: Some(0.7),
            ..CellLookup::default()
        })?;

        assert_eq!(vec![expected], cells);

        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_epistemic_pressure_lookup_uses_persistent_index_addresses(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-pressure-index-lookup");
        let index_path = temp_persistent_index_path(&path);
        let expected = {
            let mut kernel = FileKernel::open(&path)?;
            append_committed(
                &mut kernel,
                sample_cell(
                    "project:continuitydb:persistent-pressure-unrelated",
                    0.91,
                    12,
                )?,
            )?;
            let mut pressured =
                sample_cell("project:continuitydb:persistent-pressure-lookup", 0.8, 12)?;
            pressured.set_uncertainty(EpistemicUncertainty::new(
                Confidence::new(0.7)?,
                4.0,
                "high uncertainty with violated baseline",
            )?);
            pressured.set_attention(AttentionSignal::new(0.8, 0.6, 0.75, 0.4)?);
            append_committed(&mut kernel, pressured)?
        };
        let checkpoint: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&index_path)?)?;
        assert_eq!(
            checkpoint["native_epistemic_pressure_address_count"],
            serde_json::json!(2)
        );
        assert_eq!(
            checkpoint["native_epistemic_pressures"]
                .as_array()
                .map(Vec::len),
            Some(2)
        );
        let mut reopened = FileKernel::open(&path)?;
        reopened.index.native_epistemic_pressures.clear();
        reopened.index.cells.clear();

        let cells = reopened.lookup_cells(CellLookup {
            minimum_epistemic_pressure: Some(0.55),
            ..CellLookup::default()
        })?;

        assert_eq!(vec![expected], cells);

        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_filters_by_epistemic_action() -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-epistemic-action-lookup");
        let index_path = temp_persistent_index_path(&path);
        let expected = {
            let mut kernel = FileKernel::open(&path)?;
            append_committed(
                &mut kernel,
                sample_cell("project:continuitydb:file-action-use", 0.91, 12)?,
            )?;
            let mut scavenge = sample_cell("project:continuitydb:file-action-scavenge", 0.91, 12)?;
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
            append_committed(&mut kernel, scavenge)?
        };
        let mut checkpoint: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&index_path)?)?;
        checkpoint["cell_record_count"] = serde_json::json!(0);
        checkpoint["cells"] = serde_json::json!([]);
        fs::write(&index_path, serde_json::to_vec(&checkpoint)?)?;
        let reopened = FileKernel::open(&path)?;

        let cells = reopened.lookup_cells(CellLookup {
            epistemic_action: Some(EpistemicAction::Scavenge),
            ..CellLookup::default()
        })?;

        assert_eq!(vec![expected], cells);

        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_lookup_plan_indexes_uncertainty_thresholds(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-uncertainty-lookup-plan");
        let mut kernel = FileKernel::open(&path)?;
        let mut routine = sample_cell("project:continuitydb:uncertainty-plan-routine", 0.83, 15)?;
        routine.set_uncertainty(EpistemicUncertainty::new(
            Confidence::new(0.2)?,
            0.25,
            "routine ambiguity",
        )?);
        let mut uncertain = sample_cell("project:continuitydb:uncertainty-plan-high", 0.91, 12)?;
        uncertain.set_uncertainty(EpistemicUncertainty::new(
            Confidence::new(0.9)?,
            4.0,
            "prediction failed",
        )?);
        append_committed(&mut kernel, routine)?;
        append_committed(&mut kernel, uncertain)?;

        let plan = kernel.lookup_plan(&CellLookup {
            minimum_uncertainty: Some(Confidence::new(0.7)?),
            minimum_surprise_bits: Some(3.0),
            ..CellLookup::default()
        });

        assert_eq!(
            plan.indexed_constraints,
            vec!["minimum_uncertainty", "minimum_surprise_bits"]
        );
        assert_eq!(plan.residual_exact_constraints, Vec::<&'static str>::new());
        assert_eq!(plan.candidate_count, 1);
        assert!(!plan.full_scan);

        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_filters_by_minimum_probability_delta() -> Result<(), Box<dyn std::error::Error>>
    {
        let path = temp_kernel_path("continuitydb-file-kernel-probability-delta-filter");
        let mut kernel = FileKernel::open(&path)?;
        let mut shifted = sample_cell("project:continuitydb:probability-delta-shift", 0.91, 12)?;
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
        let mut routine = sample_cell("project:continuitydb:probability-delta-routine", 0.91, 12)?;
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
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_filters_by_lifecycle_use_policy() -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-lifecycle-policy-filter");
        let index_path = temp_persistent_index_path(&path);
        let mut kernel = FileKernel::open(&path)?;
        let mut verify = sample_cell(
            "project:continuitydb:file-lifecycle-policy-verify",
            0.91,
            12,
        )?;
        verify.set_lifecycle_policy(ContextLifecyclePolicy {
            retention: RetentionPolicy::DecayUnlessReinforced,
            use_policy: UsePolicy::VerifyBeforeUse,
            promotion: PromotionPolicy::Manual,
        });
        let verify = append_committed(&mut kernel, verify)?;
        append_committed(
            &mut kernel,
            sample_cell("project:continuitydb:file-lifecycle-policy-use", 0.91, 12)?,
        )?;
        let mut reopened = FileKernel::open(&path)?;
        reopened.index.use_policies.clear();
        reopened.index.cells.clear();

        let results = reopened.lookup_cells(CellLookup {
            use_policy: Some(UsePolicy::VerifyBeforeUse),
            ..CellLookup::default()
        })?;

        assert_eq!(results, vec![verify]);
        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_lookup_plan_indexes_lifecycle_policy() -> Result<(), Box<dyn std::error::Error>>
    {
        let path = temp_kernel_path("continuitydb-file-kernel-lifecycle-policy-plan");
        let mut kernel = FileKernel::open(&path)?;
        append_committed(
            &mut kernel,
            sample_cell("project:continuitydb:lifecycle-policy-plan-use", 0.83, 15)?,
        )?;
        let mut verify = sample_cell(
            "project:continuitydb:lifecycle-policy-plan-verify",
            0.91,
            12,
        )?;
        verify.set_lifecycle_policy(ContextLifecyclePolicy {
            retention: RetentionPolicy::DecayUnlessReinforced,
            use_policy: UsePolicy::VerifyBeforeUse,
            promotion: PromotionPolicy::Manual,
        });
        append_committed(&mut kernel, verify)?;

        let plan = kernel.lookup_plan(&CellLookup {
            use_policy: Some(UsePolicy::VerifyBeforeUse),
            ..CellLookup::default()
        });

        assert_eq!(plan.indexed_constraints, vec!["use_policy"]);
        assert_eq!(plan.residual_exact_constraints, Vec::<&'static str>::new());
        assert_eq!(plan.candidate_count, 1);
        assert!(!plan.full_scan);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rebuilds_tampered_lifecycle_policy_persistent_index_key(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-tampered-lifecycle-policy-key");
        let index_path = temp_persistent_index_path(&path);
        let committed_at = test_commit_time()?;
        let mut cell = sample_cell(
            "project:continuitydb:tampered-lifecycle-policy-key",
            0.91,
            12,
        )?;
        cell.set_lifecycle_policy(ContextLifecyclePolicy {
            retention: RetentionPolicy::DecayUnlessReinforced,
            use_policy: UsePolicy::VerifyBeforeUse,
            promotion: PromotionPolicy::Manual,
        });
        let expected_id = cell.id;
        let mut kernel = FileKernel::open(&path)?;
        kernel.append_cells_at(vec![cell], committed_at)?;

        let mut checkpoint: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&index_path)?)?;
        checkpoint["use_policies"][0]["policy"] = serde_json::to_value(UsePolicy::UseDirectly)?;
        fs::write(&index_path, serde_json::to_vec_pretty(&checkpoint)?)?;
        kernel.index.use_policies.clear();
        kernel.index.cells.clear();

        let results = kernel.lookup_cells(CellLookup {
            use_policy: Some(UsePolicy::VerifyBeforeUse),
            ..CellLookup::default()
        })?;

        assert_eq!(
            results.iter().map(|cell| cell.id).collect::<Vec<_>>(),
            vec![expected_id]
        );
        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_lookup_plan_reports_probability_delta_as_residual_exact(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-probability-delta-plan");
        let kernel = FileKernel::open(&path)?;

        let plan = kernel.lookup_plan(&CellLookup {
            minimum_probability_delta: Some(0.7),
            ..CellLookup::default()
        });

        assert_eq!(plan.exact_constraints, vec!["minimum_probability_delta"]);
        assert_eq!(
            plan.residual_exact_constraints,
            vec!["minimum_probability_delta"]
        );
        assert!(plan.full_scan);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_lookup_plan_indexes_salience_threshold() -> Result<(), Box<dyn std::error::Error>>
    {
        let path = temp_kernel_path("continuitydb-file-kernel-salience-lookup-plan");
        let mut kernel = FileKernel::open(&path)?;
        let mut routine = sample_cell("project:continuitydb:salience-plan-routine", 0.83, 15)?;
        routine.set_attention(AttentionSignal::new(0.1, 0.1, 0.2, 0.0)?);
        let mut salient = sample_cell("project:continuitydb:salience-plan-high", 0.91, 12)?;
        salient.set_attention(AttentionSignal::new(0.9, 0.8, 0.9, 0.8)?);
        append_committed(&mut kernel, routine)?;
        append_committed(&mut kernel, salient)?;

        let plan = kernel.lookup_plan(&CellLookup {
            minimum_salience: Some(0.7),
            ..CellLookup::default()
        });

        assert_eq!(plan.indexed_constraints, vec!["minimum_salience"]);
        assert_eq!(plan.residual_exact_constraints, Vec::<&'static str>::new());
        assert_eq!(plan.candidate_count, 1);
        assert!(!plan.full_scan);

        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_filters_by_minimum_context_affordance() -> Result<(), Box<dyn std::error::Error>>
    {
        let path = temp_kernel_path("continuitydb-file-kernel-context-affordance-filter");
        let mut kernel = FileKernel::open(&path)?;
        let mut high_value = sample_cell("project:continuitydb:affordance-file-high", 0.91, 12)?;
        high_value.set_context_affordance(ContextAffordance::new(0.95, 0.9, 0.8, 0.4, 0.95, 0.1)?);
        let high_value = append_committed(&mut kernel, high_value)?;
        let mut routine = sample_cell("project:continuitydb:affordance-file-routine", 0.91, 12)?;
        routine.set_context_affordance(ContextAffordance::new(0.2, 0.1, 0.1, 0.1, 0.2, 0.2)?);
        append_committed(&mut kernel, routine)?;

        let results = kernel.lookup_cells(CellLookup {
            minimum_context_affordance: Some(0.7),
            ..CellLookup::default()
        })?;

        assert_eq!(results, vec![high_value]);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_context_affordance_threshold_lookup_uses_persistent_index_addresses(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-affordance-index-lookup");
        let index_path = temp_persistent_index_path(&path);
        let expected = {
            let mut kernel = FileKernel::open(&path)?;
            let mut routine = sample_cell(
                "project:continuitydb:persistent-affordance-unrelated",
                0.83,
                15,
            )?;
            routine.set_context_affordance(ContextAffordance::new(0.2, 0.1, 0.1, 0.1, 0.2, 0.2)?);
            let mut high_value = sample_cell(
                "project:continuitydb:persistent-affordance-lookup",
                0.91,
                12,
            )?;
            high_value
                .set_context_affordance(ContextAffordance::new(0.95, 0.9, 0.8, 0.4, 0.95, 0.1)?);
            append_committed(&mut kernel, routine)?;
            append_committed(&mut kernel, high_value)?
        };
        let mut reopened = FileKernel::open(&path)?;
        reopened.index.native_context_affordances.clear();
        reopened.index.cells.clear();

        let cells = reopened.lookup_cells(CellLookup {
            minimum_context_affordance: Some(0.7),
            ..CellLookup::default()
        })?;

        assert_eq!(vec![expected], cells);

        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_lookup_plan_indexes_context_affordance_threshold(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-context-affordance-lookup-plan");
        let mut kernel = FileKernel::open(&path)?;
        let mut routine = sample_cell("project:continuitydb:affordance-plan-routine", 0.83, 15)?;
        routine.set_context_affordance(ContextAffordance::new(0.2, 0.1, 0.1, 0.1, 0.2, 0.2)?);
        let mut high_value = sample_cell("project:continuitydb:affordance-plan-high", 0.91, 12)?;
        high_value.set_context_affordance(ContextAffordance::new(0.95, 0.9, 0.8, 0.4, 0.95, 0.1)?);
        append_committed(&mut kernel, routine)?;
        append_committed(&mut kernel, high_value)?;

        let plan = kernel.lookup_plan(&CellLookup {
            minimum_context_affordance: Some(0.7),
            ..CellLookup::default()
        });

        assert_eq!(plan.indexed_constraints, vec!["minimum_context_affordance"]);
        assert_eq!(plan.residual_exact_constraints, Vec::<&'static str>::new());
        assert_eq!(plan.candidate_count, 1);
        assert!(!plan.full_scan);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rebuilds_tampered_context_affordance_persistent_index_key(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-tampered-context-affordance-key");
        let index_path = temp_persistent_index_path(&path);
        let committed_at = test_commit_time()?;
        let mut cell = sample_cell(
            "project:continuitydb:tampered-context-affordance-key",
            0.91,
            12,
        )?;
        cell.set_context_affordance(ContextAffordance::new(0.95, 0.9, 0.8, 0.4, 0.95, 0.1)?);
        let expected_id = cell.id;
        let mut kernel = FileKernel::open(&path)?;
        kernel.append_cells_at(vec![cell], committed_at)?;

        let mut checkpoint: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&index_path)?)?;
        checkpoint["native_context_affordances"][0]["context_affordance_microunits"] =
            serde_json::json!(1);
        fs::write(&index_path, serde_json::to_vec_pretty(&checkpoint)?)?;
        kernel.index.native_context_affordances.clear();
        kernel.index.cells.clear();

        let results = kernel.lookup_cells(CellLookup {
            minimum_context_affordance: Some(0.7),
            ..CellLookup::default()
        })?;

        assert_eq!(
            results.iter().map(|cell| cell.id).collect::<Vec<_>>(),
            vec![expected_id]
        );
        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_lookup_plan_indexes_epistemic_action() -> Result<(), Box<dyn std::error::Error>>
    {
        let path = temp_kernel_path("continuitydb-file-kernel-epistemic-action-lookup-plan");
        let mut kernel = FileKernel::open(&path)?;
        let mut scavenge = sample_cell(
            "project:continuitydb:epistemic-action-plan-scavenge",
            0.91,
            12,
        )?;
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
        append_committed(&mut kernel, scavenge)?;

        let plan = kernel.lookup_plan(&CellLookup {
            epistemic_action: Some(EpistemicAction::Scavenge),
            ..CellLookup::default()
        });

        assert_eq!(plan.indexed_constraints, vec!["epistemic_action"]);
        assert_eq!(plan.residual_exact_constraints, Vec::<&'static str>::new());
        assert_eq!(plan.candidate_count, 1);
        assert!(!plan.full_scan);

        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_lookup_plan_indexes_epistemic_pressure_threshold(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-pressure-lookup-plan");
        let mut kernel = FileKernel::open(&path)?;
        let mut routine = sample_cell("project:continuitydb:pressure-plan-routine", 0.91, 12)?;
        routine.set_uncertainty(EpistemicUncertainty::new(
            Confidence::new(0.2)?,
            0.2,
            "minor uncertainty",
        )?);
        let mut pressured = sample_cell("project:continuitydb:pressure-plan-high", 0.8, 12)?;
        pressured.set_uncertainty(EpistemicUncertainty::new(
            Confidence::new(0.7)?,
            4.0,
            "high uncertainty with violated baseline",
        )?);
        pressured.set_attention(AttentionSignal::new(0.8, 0.6, 0.75, 0.4)?);
        append_committed(&mut kernel, routine)?;
        append_committed(&mut kernel, pressured)?;

        let plan = kernel.lookup_plan(&CellLookup {
            minimum_epistemic_pressure: Some(0.55),
            ..CellLookup::default()
        });

        assert_eq!(plan.indexed_constraints, vec!["minimum_epistemic_pressure"]);
        assert_eq!(plan.residual_exact_constraints, Vec::<&'static str>::new());
        assert_eq!(plan.candidate_count, 1);
        assert!(!plan.full_scan);

        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rebuilds_tampered_uncertainty_persistent_index_key(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-tampered-uncertainty-index-key");
        let index_path = temp_persistent_index_path(&path);
        let committed_at = test_commit_time()?;
        let mut cell = sample_cell(
            "project:continuitydb:tampered-uncertainty-index-key",
            0.91,
            12,
        )?;
        cell.set_uncertainty(EpistemicUncertainty::new(
            Confidence::new(0.88)?,
            4.0,
            "frontier baseline failed",
        )?);
        let expected_id = cell.id;
        let mut kernel = FileKernel::open(&path)?;
        kernel.append_cells_at(vec![cell], committed_at)?;

        let mut checkpoint: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&index_path)?)?;
        checkpoint["native_uncertainties"][0]["uncertainty_microunits"] = serde_json::json!(1);
        fs::write(&index_path, serde_json::to_vec_pretty(&checkpoint)?)?;
        kernel.index.native_uncertainties.clear();
        kernel.index.cells.clear();

        let results = kernel.lookup_cells(CellLookup {
            minimum_uncertainty: Some(Confidence::new(0.7)?),
            ..CellLookup::default()
        })?;

        assert_eq!(
            results.iter().map(|cell| cell.id).collect::<Vec<_>>(),
            vec![expected_id]
        );
        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rebuilds_tampered_salience_persistent_index_key(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-tampered-salience-index-key");
        let index_path = temp_persistent_index_path(&path);
        let committed_at = test_commit_time()?;
        let mut cell = sample_cell("project:continuitydb:tampered-salience-index-key", 0.91, 12)?;
        cell.set_attention(AttentionSignal::new(0.9, 0.8, 0.9, 0.8)?);
        let expected_id = cell.id;
        let mut kernel = FileKernel::open(&path)?;
        kernel.append_cells_at(vec![cell], committed_at)?;

        let mut checkpoint: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&index_path)?)?;
        checkpoint["native_saliences"][0]["salience_microunits"] = serde_json::json!(1);
        fs::write(&index_path, serde_json::to_vec_pretty(&checkpoint)?)?;
        kernel.index.native_saliences.clear();
        kernel.index.cells.clear();

        let results = kernel.lookup_cells(CellLookup {
            minimum_salience: Some(0.7),
            ..CellLookup::default()
        })?;

        assert_eq!(
            results.iter().map(|cell| cell.id).collect::<Vec<_>>(),
            vec![expected_id]
        );
        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rebuilds_tampered_epistemic_action_persistent_index_key(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-tampered-action-index-key");
        let index_path = temp_persistent_index_path(&path);
        let committed_at = test_commit_time()?;
        let mut cell = sample_cell("project:continuitydb:tampered-action-index-key", 0.91, 12)?;
        let expectation = EpistemicExpectation::from_expected_outcome(
            "release asset exists",
            Confidence::new(0.9)?,
            false,
        )?;
        cell.set_uncertainty(EpistemicUncertainty::from_expectation(
            Confidence::new(0.82)?,
            expectation,
            "strong baseline failed and needs missing evidence",
        )?);
        let expected_id = cell.id;
        let mut kernel = FileKernel::open(&path)?;
        kernel.append_cells_at(vec![cell], committed_at)?;

        let mut checkpoint: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&index_path)?)?;
        checkpoint["epistemic_actions"][0]["action"] = serde_json::to_value(EpistemicAction::Use)?;
        fs::write(&index_path, serde_json::to_vec_pretty(&checkpoint)?)?;
        kernel.index.epistemic_actions.clear();
        kernel.index.cells.clear();

        let results = kernel.lookup_cells(CellLookup {
            epistemic_action: Some(EpistemicAction::Scavenge),
            ..CellLookup::default()
        })?;

        assert_eq!(
            results.iter().map(|cell| cell.id).collect::<Vec<_>>(),
            vec![expected_id]
        );
        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rebuilds_tampered_epistemic_action_reason_persistent_index_key(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-tampered-action-reason-index-key");
        let index_path = temp_persistent_index_path(&path);
        let committed_at = test_commit_time()?;
        let mut cell = sample_cell(
            "project:continuitydb:tampered-action-reason-index-key",
            0.91,
            12,
        )?;
        let expectation = EpistemicExpectation::from_expected_outcome(
            "release asset exists",
            Confidence::new(0.9)?,
            false,
        )?;
        cell.set_uncertainty(EpistemicUncertainty::from_expectation(
            Confidence::new(0.22)?,
            expectation,
            "baseline failed but confidence remains low",
        )?);
        let expected_id = cell.id;
        let mut kernel = FileKernel::open(&path)?;
        kernel.append_cells_at(vec![cell], committed_at)?;

        let mut checkpoint: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&index_path)?)?;
        checkpoint["epistemic_action_reasons"][0]["reason"] =
            serde_json::to_value(EpistemicActionReason::HighUncertainty)?;
        fs::write(&index_path, serde_json::to_vec_pretty(&checkpoint)?)?;
        kernel.index.epistemic_action_reasons.clear();
        kernel.index.cells.clear();

        let results = kernel.lookup_cells(CellLookup {
            epistemic_action_reason: Some(EpistemicActionReason::HighSurprise),
            ..CellLookup::default()
        })?;

        assert_eq!(
            results.iter().map(|cell| cell.id).collect::<Vec<_>>(),
            vec![expected_id]
        );
        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rebuilds_tampered_epistemic_pressure_persistent_index_key(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-tampered-pressure-index-key");
        let index_path = temp_persistent_index_path(&path);
        let committed_at = test_commit_time()?;
        let mut cell = sample_cell("project:continuitydb:tampered-pressure-index-key", 0.8, 12)?;
        cell.set_uncertainty(EpistemicUncertainty::new(
            Confidence::new(0.7)?,
            4.0,
            "high uncertainty with violated baseline",
        )?);
        cell.set_attention(AttentionSignal::new(0.8, 0.6, 0.75, 0.4)?);
        let expected_id = cell.id;
        let mut kernel = FileKernel::open(&path)?;
        kernel.append_cells_at(vec![cell], committed_at)?;

        let mut checkpoint: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&index_path)?)?;
        checkpoint["native_epistemic_pressures"][0]["pressure_microunits"] = serde_json::json!(1);
        fs::write(&index_path, serde_json::to_vec_pretty(&checkpoint)?)?;
        kernel.index.native_epistemic_pressures.clear();
        kernel.index.cells.clear();

        let results = kernel.lookup_cells(CellLookup {
            minimum_epistemic_pressure: Some(0.55),
            ..CellLookup::default()
        })?;

        assert_eq!(
            results.iter().map(|cell| cell.id).collect::<Vec<_>>(),
            vec![expected_id]
        );
        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rebuilds_tampered_surprise_persistent_index_key(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-tampered-surprise-index-key");
        let index_path = temp_persistent_index_path(&path);
        let committed_at = test_commit_time()?;
        let mut cell = sample_cell("project:continuitydb:tampered-surprise-index-key", 0.91, 12)?;
        cell.set_uncertainty(EpistemicUncertainty::new(
            Confidence::new(0.55)?,
            5.5,
            "high-certainty expectation failed",
        )?);
        let expected_id = cell.id;
        let mut kernel = FileKernel::open(&path)?;
        kernel.append_cells_at(vec![cell], committed_at)?;

        let mut checkpoint: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&index_path)?)?;
        checkpoint["native_surprise_bits"][0]["surprise_microbits"] = serde_json::json!(1);
        fs::write(&index_path, serde_json::to_vec_pretty(&checkpoint)?)?;
        kernel.index.native_surprise_bits.clear();
        kernel.index.cells.clear();

        let results = kernel.lookup_cells(CellLookup {
            minimum_surprise_bits: Some(3.0),
            ..CellLookup::default()
        })?;

        assert_eq!(
            results.iter().map(|cell| cell.id).collect::<Vec<_>>(),
            vec![expected_id]
        );
        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_answerability_question_lookup_uses_persistent_index_addresses(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-answerability-index-lookup");
        let index_path = temp_persistent_index_path(&path);
        let expected = {
            let mut kernel = FileKernel::open(&path)?;
            let matching = StateCell {
                answerability: Answerability::new(vec![
                    "which persistent question routes to byte ranges?".to_string(),
                ])?,
                ..sample_cell(
                    "project:continuitydb:persistent-answerability-lookup",
                    0.91,
                    12,
                )?
            };
            let unrelated = StateCell {
                answerability: Answerability::new(vec![
                    "which unrelated question stays filtered?".to_string(),
                ])?,
                ..sample_cell(
                    "project:continuitydb:persistent-answerability-unrelated",
                    0.83,
                    15,
                )?
            };
            let expected = append_committed(&mut kernel, matching)?;
            append_committed(&mut kernel, unrelated)?;
            expected
        };
        let mut reopened = FileKernel::open(&path)?;
        reopened.index.answerability_questions.clear();
        reopened.index.cells.clear();

        let cells = reopened.lookup_cells(CellLookup {
            answerability_question: Some(
                "which persistent question routes to byte ranges?".to_string(),
            ),
            ..CellLookup::default()
        })?;

        assert_eq!(vec![expected], cells);

        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rebuilds_tampered_answerability_question_persistent_index_key(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-tampered-answerability-index-key");
        let index_path = temp_persistent_index_path(&path);
        let committed_at = test_commit_time()?;
        let question = "which tampered answerability key should rebuild?";
        let cell = StateCell {
            answerability: Answerability::new(vec![question.to_string()])?,
            ..sample_cell(
                "project:continuitydb:tampered-answerability-index-key",
                0.91,
                12,
            )?
        };
        let expected_id = cell.id;
        let mut kernel = FileKernel::open(&path)?;
        kernel.append_cells_at(vec![cell], committed_at)?;

        let mut checkpoint: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&index_path)?)?;
        checkpoint["answerability_questions"][0]["question"] =
            serde_json::Value::String("which wrong answerability key was stored?".to_string());
        fs::write(&index_path, serde_json::to_vec_pretty(&checkpoint)?)?;
        kernel.index.answerability_questions.clear();
        kernel.index.cells.clear();

        let results = kernel.lookup_cells(CellLookup {
            answerability_question: Some(question.to_string()),
            ..CellLookup::default()
        })?;

        assert_eq!(
            results.iter().map(|cell| cell.id).collect::<Vec<_>>(),
            vec![expected_id]
        );
        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_evidence_source_lookup_uses_persistent_index_addresses(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-evidence-source-index-lookup");
        let index_path = temp_persistent_index_path(&path);
        let expected = {
            let mut kernel = FileKernel::open(&path)?;
            let matching = sample_cell_with_source(
                "project:continuitydb:persistent-evidence-source-lookup",
                "test://persistent-evidence-source",
                0.91,
                12,
            )?;
            let unrelated = sample_cell_with_source(
                "project:continuitydb:persistent-evidence-source-unrelated",
                "test://persistent-evidence-unrelated",
                0.83,
                15,
            )?;
            let expected = append_committed(&mut kernel, matching)?;
            append_committed(&mut kernel, unrelated)?;
            expected
        };
        let mut reopened = FileKernel::open(&path)?;
        reopened.index.evidence_sources.clear();
        reopened.index.cells.clear();

        let cells = reopened.lookup_cells(CellLookup {
            evidence_source: Some("test://persistent-evidence-source".to_string()),
            ..CellLookup::default()
        })?;

        assert_eq!(vec![expected], cells);

        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rebuilds_tampered_evidence_source_persistent_index_key(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-tampered-evidence-index-key");
        let index_path = temp_persistent_index_path(&path);
        let committed_at = test_commit_time()?;
        let source = "test://tampered-evidence-source";
        let cell = sample_cell_with_source(
            "project:continuitydb:tampered-evidence-index-key",
            source,
            0.91,
            12,
        )?;
        let expected_id = cell.id;
        let mut kernel = FileKernel::open(&path)?;
        kernel.append_cells_at(vec![cell], committed_at)?;

        let mut checkpoint: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&index_path)?)?;
        checkpoint["evidence_sources"][0]["source"] =
            serde_json::Value::String("test://wrong-evidence-source".to_string());
        fs::write(&index_path, serde_json::to_vec_pretty(&checkpoint)?)?;
        kernel.index.evidence_sources.clear();
        kernel.index.cells.clear();

        let results = kernel.lookup_cells(CellLookup {
            evidence_source: Some(source.to_string()),
            ..CellLookup::default()
        })?;

        assert_eq!(
            results.iter().map(|cell| cell.id).collect::<Vec<_>>(),
            vec![expected_id]
        );
        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_minimum_confidence_lookup_uses_persistent_index_addresses(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-confidence-index-lookup");
        let index_path = temp_persistent_index_path(&path);
        let expected = {
            let mut kernel = FileKernel::open(&path)?;
            let weak = sample_cell(
                "project:continuitydb:persistent-confidence-unrelated",
                0.61,
                15,
            )?;
            let strong = sample_cell(
                "project:continuitydb:persistent-confidence-lookup",
                0.91,
                12,
            )?;
            append_committed(&mut kernel, weak)?;
            append_committed(&mut kernel, strong)?
        };
        let mut reopened = FileKernel::open(&path)?;
        reopened.index.max_evidence_confidences.clear();
        reopened.index.cells.clear();

        let cells = reopened.lookup_cells(CellLookup {
            minimum_confidence: Some(Confidence::new(0.8)?),
            ..CellLookup::default()
        })?;

        assert_eq!(vec![expected], cells);

        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rebuilds_tampered_confidence_persistent_index_key(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-tampered-confidence-index-key");
        let index_path = temp_persistent_index_path(&path);
        let committed_at = test_commit_time()?;
        let cell = sample_cell(
            "project:continuitydb:tampered-confidence-index-key",
            0.91,
            12,
        )?;
        let expected_id = cell.id;
        let mut kernel = FileKernel::open(&path)?;
        kernel.append_cells_at(vec![cell], committed_at)?;

        let mut checkpoint: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&index_path)?)?;
        checkpoint["max_evidence_confidences"][0]["confidence_microunits"] =
            serde_json::Value::from(100_000_u32);
        fs::write(&index_path, serde_json::to_vec_pretty(&checkpoint)?)?;
        kernel.index.max_evidence_confidences.clear();
        kernel.index.cells.clear();

        let results = kernel.lookup_cells(CellLookup {
            minimum_confidence: Some(Confidence::new(0.8)?),
            ..CellLookup::default()
        })?;

        assert_eq!(
            results.iter().map(|cell| cell.id).collect::<Vec<_>>(),
            vec![expected_id]
        );
        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_system_time_lookup_uses_persistent_index_addresses(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-system-time-index-lookup");
        let index_path = temp_persistent_index_path(&path);
        let first_commit = test_commit_time()?;
        let second_commit = first_commit + chrono::Duration::minutes(5);
        let as_of_first = first_commit + chrono::Duration::minutes(1);
        let first_commit_id = CommitId::new();
        let second_commit_id = CommitId::new();
        let first = sample_cell(
            "project:continuitydb:persistent-system-time-first",
            0.91,
            12,
        )?;
        let second = sample_cell(
            "project:continuitydb:persistent-system-time-second",
            0.83,
            15,
        )?;
        let expected = StateCell {
            system_time: continuitydb_core::SystemTimeRange::open_from(first_commit),
            commit_id: first_commit_id,
            ..first.clone()
        };
        {
            let mut kernel = FileKernel::open(&path)?;
            kernel.append_cell_at_with_commit_id(first, first_commit, first_commit_id)?;
            kernel.append_cell_at_with_commit_id(second, second_commit, second_commit_id)?;
        }
        let mut reopened = FileKernel::open(&path)?;
        reopened.index.system_times.clear();
        reopened.index.cells.clear();

        let cells = reopened.lookup_cells(CellLookup {
            system_at: Some(as_of_first),
            ..CellLookup::default()
        })?;

        assert_eq!(vec![expected], cells);

        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rebuilds_tampered_system_time_persistent_index_key(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-tampered-system-time-index-key");
        let index_path = temp_persistent_index_path(&path);
        let committed_at = test_commit_time()?;
        let as_of_commit = committed_at + chrono::Duration::minutes(1);
        let cell = sample_cell(
            "project:continuitydb:tampered-system-time-index-key",
            0.91,
            12,
        )?;
        let expected_id = cell.id;
        let mut kernel = FileKernel::open(&path)?;
        kernel.append_cells_at(vec![cell], committed_at)?;

        let mut checkpoint: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&index_path)?)?;
        checkpoint["system_times"][0]["system_from"] =
            serde_json::to_value(committed_at + chrono::Duration::days(1))?;
        fs::write(&index_path, serde_json::to_vec_pretty(&checkpoint)?)?;
        kernel.index.system_times.clear();
        kernel.index.cells.clear();

        let results = kernel.lookup_cells(CellLookup {
            system_at: Some(as_of_commit),
            ..CellLookup::default()
        })?;

        assert_eq!(
            results.iter().map(|cell| cell.id).collect::<Vec<_>>(),
            vec![expected_id]
        );
        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_valid_time_lookup_uses_persistent_index_addresses(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-valid-time-index-lookup");
        let index_path = temp_persistent_index_path(&path);
        let first_valid = Utc
            .with_ymd_and_hms(2026, 5, 20, 0, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let second_valid = Utc
            .with_ymd_and_hms(2026, 5, 21, 0, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let as_of_first = first_valid + chrono::Duration::hours(1);
        let matching = StateCell {
            valid_time: ValidTimeRange::new(first_valid, None)?,
            ..sample_cell("project:continuitydb:persistent-valid-time-first", 0.91, 12)?
        };
        let later = StateCell {
            valid_time: ValidTimeRange::new(second_valid, None)?,
            ..sample_cell(
                "project:continuitydb:persistent-valid-time-second",
                0.83,
                15,
            )?
        };
        let expected = {
            let mut kernel = FileKernel::open(&path)?;
            let expected = append_committed(&mut kernel, matching)?;
            append_committed(&mut kernel, later)?;
            expected
        };
        let mut reopened = FileKernel::open(&path)?;
        reopened.index.valid_times.clear();
        reopened.index.cells.clear();

        let cells = reopened.lookup_cells(CellLookup {
            valid_at: Some(as_of_first),
            ..CellLookup::default()
        })?;

        assert_eq!(vec![expected], cells);

        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rebuilds_tampered_valid_time_persistent_index_key(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-tampered-valid-time-index-key");
        let index_path = temp_persistent_index_path(&path);
        let committed_at = test_commit_time()?;
        let valid_from = Utc
            .with_ymd_and_hms(2026, 5, 20, 0, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let as_of_valid = valid_from + chrono::Duration::hours(1);
        let cell = StateCell {
            valid_time: ValidTimeRange::new(valid_from, None)?,
            ..sample_cell(
                "project:continuitydb:tampered-valid-time-index-key",
                0.91,
                12,
            )?
        };
        let expected_id = cell.id;
        let mut kernel = FileKernel::open(&path)?;
        kernel.append_cells_at(vec![cell], committed_at)?;

        let mut checkpoint: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&index_path)?)?;
        checkpoint["valid_times"][0]["valid_from"] =
            serde_json::to_value(valid_from + chrono::Duration::days(1))?;
        fs::write(&index_path, serde_json::to_vec_pretty(&checkpoint)?)?;
        kernel.index.valid_times.clear();
        kernel.index.cells.clear();

        let results = kernel.lookup_cells(CellLookup {
            valid_at: Some(as_of_valid),
            ..CellLookup::default()
        })?;

        assert_eq!(
            results.iter().map(|cell| cell.id).collect::<Vec<_>>(),
            vec![expected_id]
        );
        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_dependency_target_lookup_uses_persistent_index_addresses(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-dependency-target-index-lookup");
        let index_path = temp_persistent_index_path(&path);
        let target = StateCellId::new();
        let other_target = StateCellId::new();
        let expected = {
            let mut kernel = FileKernel::open(&path)?;
            let mut dependent = sample_cell(
                "project:continuitydb:persistent-dependency-target",
                0.91,
                12,
            )?;
            dependent.dependencies.push(CellDependency::new(
                target,
                CellDependencyKind::DependsOn,
                "depends on target",
            ));
            let mut unrelated = sample_cell(
                "project:continuitydb:persistent-dependency-unrelated",
                0.83,
                15,
            )?;
            unrelated.dependencies.push(CellDependency::new(
                other_target,
                CellDependencyKind::DependsOn,
                "depends on another target",
            ));
            let expected = append_committed(&mut kernel, dependent)?;
            append_committed(&mut kernel, unrelated)?;
            expected
        };
        let mut reopened = FileKernel::open(&path)?;
        reopened.index.dependency_targets.clear();
        reopened.index.cells.clear();

        let cells = reopened.lookup_cells(CellLookup {
            dependency_target: Some(target),
            ..CellLookup::default()
        })?;

        assert_eq!(vec![expected], cells);

        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rebuilds_tampered_dependency_target_persistent_index_key(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-tampered-dependency-target-key");
        let index_path = temp_persistent_index_path(&path);
        let committed_at = test_commit_time()?;
        let target = StateCellId::new();
        let mut cell = sample_cell(
            "project:continuitydb:tampered-dependency-target-key",
            0.91,
            12,
        )?;
        cell.dependencies.push(CellDependency::new(
            target,
            CellDependencyKind::DependsOn,
            "depends on target",
        ));
        let expected_id = cell.id;
        let mut kernel = FileKernel::open(&path)?;
        kernel.append_cells_at(vec![cell], committed_at)?;

        let mut checkpoint: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&index_path)?)?;
        checkpoint["dependency_targets"][0]["target"] = serde_json::to_value(StateCellId::new())?;
        fs::write(&index_path, serde_json::to_vec_pretty(&checkpoint)?)?;
        kernel.index.dependency_targets.clear();
        kernel.index.cells.clear();

        let results = kernel.lookup_cells(CellLookup {
            dependency_target: Some(target),
            ..CellLookup::default()
        })?;

        assert_eq!(
            results.iter().map(|cell| cell.id).collect::<Vec<_>>(),
            vec![expected_id]
        );
        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_dependency_kind_lookup_uses_persistent_index_addresses(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-dependency-kind-index-lookup");
        let index_path = temp_persistent_index_path(&path);
        let target = StateCellId::new();
        let other_target = StateCellId::new();
        let expected = {
            let mut kernel = FileKernel::open(&path)?;
            let mut first = sample_cell(
                "project:continuitydb:persistent-dependency-kind-first",
                0.91,
                12,
            )?;
            first.dependencies.push(CellDependency::new(
                target,
                CellDependencyKind::DependsOn,
                "depends on target",
            ));
            let mut support = sample_cell(
                "project:continuitydb:persistent-dependency-kind-support",
                0.88,
                13,
            )?;
            support.dependencies.push(CellDependency::new(
                target,
                CellDependencyKind::Supports,
                "supports target",
            ));
            let mut second = sample_cell(
                "project:continuitydb:persistent-dependency-kind-second",
                0.86,
                15,
            )?;
            second.dependencies.push(CellDependency::new(
                other_target,
                CellDependencyKind::DependsOn,
                "depends on another target",
            ));
            let first = append_committed(&mut kernel, first)?;
            append_committed(&mut kernel, support)?;
            let second = append_committed(&mut kernel, second)?;
            vec![first, second]
        };
        let mut reopened = FileKernel::open(&path)?;
        reopened.index.dependency_kinds.clear();
        reopened.index.cells.clear();

        let cells = reopened.lookup_cells(CellLookup {
            dependency_kind: Some(CellDependencyKind::DependsOn),
            ..CellLookup::default()
        })?;

        assert_eq!(expected, cells);

        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rebuilds_tampered_dependency_kind_persistent_index_key(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-tampered-dependency-kind-key");
        let index_path = temp_persistent_index_path(&path);
        let committed_at = test_commit_time()?;
        let target = StateCellId::new();
        let mut cell = sample_cell(
            "project:continuitydb:tampered-dependency-kind-key",
            0.91,
            12,
        )?;
        cell.dependencies.push(CellDependency::new(
            target,
            CellDependencyKind::DependsOn,
            "depends on target",
        ));
        let expected_id = cell.id;
        let mut kernel = FileKernel::open(&path)?;
        kernel.append_cells_at(vec![cell], committed_at)?;

        let mut checkpoint: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&index_path)?)?;
        checkpoint["dependency_kinds"][0]["kind"] =
            serde_json::to_value(CellDependencyKind::Supports)?;
        fs::write(&index_path, serde_json::to_vec_pretty(&checkpoint)?)?;
        kernel.index.dependency_kinds.clear();
        kernel.index.cells.clear();

        let results = kernel.lookup_cells(CellLookup {
            dependency_kind: Some(CellDependencyKind::DependsOn),
            ..CellLookup::default()
        })?;

        assert_eq!(
            results.iter().map(|cell| cell.id).collect::<Vec<_>>(),
            vec![expected_id]
        );
        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    fn indexed_revision_links(
        index: &super::FileKernelIndex,
        positions: &[usize],
    ) -> Vec<RevisionLinkRecord> {
        positions
            .iter()
            .map(|position| index.revision_links[*position].clone())
            .collect()
    }

    #[test]
    fn default_capabilities_are_ephemeral() {
        let capabilities = KernelCapabilities::ephemeral();

        assert_eq!(KernelDurability::Ephemeral, capabilities.durability);
        assert!(capabilities.append_only);
        assert!(!capabilities.derived_indexes);
        assert!(!capabilities.persistent_indexes);
        assert!(!capabilities.explicit_commit_records);
        assert!(!capabilities.durable_flush);
        assert!(!capabilities.compaction);
    }

    #[test]
    fn file_kernel_reports_append_log_capabilities() -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("file-kernel-capabilities");
        let kernel = FileKernel::open(&path)?;

        let capabilities = kernel.capabilities();

        assert_eq!(KernelDurability::AppendLog, capabilities.durability);
        assert!(capabilities.append_only);
        assert!(capabilities.derived_indexes);
        assert!(capabilities.persistent_indexes);
        assert!(capabilities.explicit_commit_records);
        assert!(capabilities.durable_flush);
        assert!(capabilities.compaction);

        let _ = std::fs::remove_file(path);
        Ok(())
    }

    #[test]
    fn file_kernel_status_reports_empty_store() -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-status-empty");
        let kernel = FileKernel::open(&path)?;

        let status = kernel.status()?;

        assert_eq!(status.cell_count, 0);
        assert_eq!(status.commit_count, 0);
        assert_eq!(status.revision_link_count, 0);
        assert!(status.file_size_bytes > 0);

        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_status_reports_visible_cells_commits_and_revision_links(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-status-populated");
        let committed_at = test_commit_time()?;
        let commit_id = CommitId::new();
        let first = sample_cell("project:continuitydb:status-first", 0.91, 12)?;
        let second = sample_cell("project:continuitydb:status-second", 0.83, 15)?;
        let first_id = first.id;
        let second_id = second.id;
        let mut kernel = FileKernel::open(&path)?;
        kernel.append_cells_at_with_commit_id(vec![first, second], committed_at, commit_id)?;
        kernel.append_revision_link(RevisionLinkRecord::new(
            first_id,
            RevisionLinkKind::Supersedes,
            second_id,
            committed_at,
        ))?;

        let status = kernel.status()?;

        assert_eq!(status.cell_count, 2);
        assert_eq!(status.commit_count, 1);
        assert_eq!(status.revision_link_count, 1);
        assert!(status.file_size_bytes > 0);

        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_status_uses_persistent_index_counts() -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-status-persistent-index");
        let index_path = temp_persistent_index_path(&path);
        let committed_at = test_commit_time()?;
        let commit_id = CommitId::new();
        let first = sample_cell("project:continuitydb:status-index-first", 0.91, 12)?;
        let second = sample_cell("project:continuitydb:status-index-second", 0.83, 15)?;
        let first_id = first.id;
        let second_id = second.id;
        {
            let mut kernel = FileKernel::open(&path)?;
            kernel.append_cells_at_with_commit_id(vec![first, second], committed_at, commit_id)?;
            kernel.append_revision_link(RevisionLinkRecord::new(
                first_id,
                RevisionLinkKind::Supersedes,
                second_id,
                committed_at,
            ))?;
        }
        let mut reopened = FileKernel::open(&path)?;
        reopened.index.cells.clear();
        reopened.index.manifest_order.clear();
        reopened.index.revision_links.clear();

        let status = reopened.status()?;

        assert_eq!(status.cell_count, 2);
        assert_eq!(status.commit_count, 1);
        assert_eq!(status.revision_link_count, 1);
        assert!(status.file_size_bytes > 0);

        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_health_reports_new_store_as_canonical() -> Result<(), Box<dyn std::error::Error>>
    {
        let path = temp_kernel_path("continuitydb-file-kernel-health-new");
        let kernel = FileKernel::open(&path)?;

        let health = kernel.health();

        assert!(health.has_header);
        assert_eq!(health.legacy_raw_cells, 0);
        assert_eq!(health.checksum_free_records, 0);
        assert_eq!(health.canonical_records, 0);
        assert!(!health.compaction_recommended);

        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_health_recommends_compaction_for_legacy_raw_cells(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-health-legacy");
        let committed_at = test_commit_time()?;
        let commit_id = CommitId::new();
        let mut cell = sample_cell("project:continuitydb:health-legacy", 0.91, 12)?;
        cell.system_time = continuitydb_core::SystemTimeRange::open_from(committed_at);
        cell.commit_id = commit_id;
        fs::write(&path, format!("{}\n", serde_json::to_string(&cell)?))?;

        let kernel = FileKernel::open(&path)?;
        let health = kernel.health();

        assert!(!health.has_header);
        assert_eq!(health.legacy_raw_cells, 1);
        assert_eq!(health.checksum_free_records, 0);
        assert_eq!(health.canonical_records, 0);
        assert!(health.compaction_recommended);

        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_health_recommends_compaction_for_checksum_free_records(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-health-checksum-free");
        let committed_at = test_commit_time()?;
        let commit_id = CommitId::new();
        let mut cell = sample_cell("project:continuitydb:health-checksum-free", 0.91, 12)?;
        cell.system_time = continuitydb_core::SystemTimeRange::open_from(committed_at);
        cell.commit_id = commit_id;
        let manifest = continuitydb_core::CommitManifest {
            commit_id,
            committed_at,
            cell_ids: vec![cell.id],
        };
        fs::write(
            &path,
            format!(
                "{}\n{}\n{}\n",
                serde_json::json!({
                    "type": "header",
                    "format": "continuitydb.file_kernel",
                    "version": 1
                }),
                serde_json::json!({
                    "type": "cell",
                    "cell": cell
                }),
                serde_json::json!({
                    "type": "commit",
                    "manifest": manifest
                }),
            ),
        )?;

        let kernel = FileKernel::open(&path)?;
        let health = kernel.health();

        assert!(health.has_header);
        assert_eq!(health.legacy_raw_cells, 0);
        assert_eq!(health.checksum_free_records, 2);
        assert_eq!(health.canonical_records, 0);
        assert!(health.compaction_recommended);

        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_compaction_updates_health_to_canonical() -> Result<(), Box<dyn std::error::Error>>
    {
        let path = temp_kernel_path("continuitydb-file-kernel-health-compact");
        let committed_at = test_commit_time()?;
        let commit_id = CommitId::new();
        let mut cell = sample_cell("project:continuitydb:health-compact", 0.91, 12)?;
        cell.system_time = continuitydb_core::SystemTimeRange::open_from(committed_at);
        cell.commit_id = commit_id;
        fs::write(&path, format!("{}\n", serde_json::to_string(&cell)?))?;
        let mut kernel = FileKernel::open(&path)?;

        kernel.compact()?;
        let health = kernel.health();

        assert!(health.has_header);
        assert_eq!(health.legacy_raw_cells, 0);
        assert_eq!(health.checksum_free_records, 0);
        assert_eq!(health.canonical_records, 2);
        assert!(!health.compaction_recommended);

        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn ephemeral_capabilities_satisfy_ephemeral_requirements() {
        assert!(KernelCapabilities::ephemeral().satisfies(KernelRequirements::ephemeral()));
    }

    #[test]
    fn ephemeral_capabilities_do_not_satisfy_durable_append_log_requirements() {
        assert!(
            !KernelCapabilities::ephemeral().satisfies(KernelRequirements::durable_append_log())
        );
    }

    #[test]
    fn file_append_log_capabilities_satisfy_durable_append_log_requirements() {
        assert!(KernelCapabilities::file_append_log()
            .satisfies(KernelRequirements::durable_append_log()));
    }

    #[test]
    fn file_append_log_capabilities_satisfy_persistent_indexed_append_log_requirements() {
        assert!(KernelCapabilities::file_append_log()
            .satisfies(KernelRequirements::persistent_indexed_append_log()));
    }

    #[test]
    fn file_append_log_capabilities_do_not_satisfy_indexed_embedded_requirements() {
        assert!(!KernelCapabilities::file_append_log()
            .satisfies(KernelRequirements::indexed_embedded()));
        assert!(KernelCapabilities::file_append_log().persistent_indexes);
    }

    #[test]
    fn file_kernel_durable_write_helper_persists_bytes() -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-durable-write");
        {
            let mut file = std::fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&path)?;
            write_all_durable(&mut file, b"{\"type\":\"test\"}\n")?;
        }

        assert_eq!(fs::read_to_string(&path)?, "{\"type\":\"test\"}\n");
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_parent_directory_sync_accepts_existing_parent(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-sync-parent");
        fs::write(&path, "")?;

        sync_parent_directory(&path)?;

        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_persists_cells_across_reopen() -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel");
        let cell = sample_cell("project:continuitydb:durable", 0.91, 12)?;
        let expected;
        {
            let mut kernel = FileKernel::open(&path)?;
            expected = append_committed(&mut kernel, cell)?;
        }

        let reopened = FileKernel::open(&path)?;
        let results = reopened.lookup_cells(CellLookup {
            semantic_anchor: Some("project:continuitydb:durable".to_string()),
            ..CellLookup::default()
        })?;

        assert_eq!(results, vec![expected]);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rejects_duplicate_after_reopen() -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-duplicate");
        let cell = sample_cell("project:continuitydb:duplicate", 0.91, 12)?;
        {
            let mut kernel = FileKernel::open(&path)?;
            append_committed(&mut kernel, cell.clone())?;
        }

        let mut reopened = FileKernel::open(&path)?;
        let result = reopened.append_cell(cell);

        assert!(matches!(result, Err(KernelError::DuplicateCell)));
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_duplicate_cell_guard_uses_persistent_index_addresses(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-duplicate-cell-persistent-index");
        let index_path = temp_persistent_index_path(&path);
        let cell = sample_cell("project:continuitydb:duplicate-persistent-index", 0.91, 12)?;
        {
            let mut kernel = FileKernel::open(&path)?;
            append_committed(&mut kernel, cell.clone())?;
        }
        let mut reopened = FileKernel::open(&path)?;
        reopened.index.ids.clear();
        reopened.index.cells.clear();

        let result = reopened.append_cell(cell);

        assert!(matches!(result, Err(KernelError::DuplicateCell)));
        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_persists_batch_across_reopen_with_shared_system_time(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-batch");
        let committed_at = test_commit_time()?;
        let first = sample_cell("project:continuitydb:batch-first", 0.91, 12)?;
        let second = sample_cell("project:continuitydb:batch-second", 0.83, 15)?;
        let first_id = first.id;
        let second_id = second.id;
        {
            let mut kernel = FileKernel::open(&path)?;
            kernel.append_cells_at(vec![first, second], committed_at)?;
        }

        let reopened = FileKernel::open(&path)?;
        let results = reopened.lookup_cells(CellLookup::default())?;

        assert_eq!(
            results.iter().map(|cell| cell.id).collect::<Vec<_>>(),
            vec![first_id, second_id]
        );
        assert!(results
            .iter()
            .all(|cell| cell.system_time.from() == committed_at));
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_unfiltered_lookup_uses_persistent_index_addresses(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-unfiltered-lookup-index");
        let index_path = temp_persistent_index_path(&path);
        let committed_at = test_commit_time()?;
        let first = sample_cell("project:continuitydb:unfiltered-index-first", 0.91, 12)?;
        let second = sample_cell("project:continuitydb:unfiltered-index-second", 0.83, 15)?;
        let expected_ids = vec![first.id, second.id];
        {
            let mut kernel = FileKernel::open(&path)?;
            kernel.append_cells_at(vec![first, second], committed_at)?;
        }
        let mut reopened = FileKernel::open(&path)?;
        reopened.index.cells.clear();

        let results = reopened.lookup_cells(CellLookup::default())?;

        assert_eq!(
            results.iter().map(|cell| cell.id).collect::<Vec<_>>(),
            expected_ids
        );
        assert!(results
            .iter()
            .all(|cell| cell.system_time.from() == committed_at));
        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rebuilds_stale_persistent_index_checkpoint(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-stale-persistent-index");
        let index_path = temp_persistent_index_path(&path);
        let committed_at = test_commit_time()?;
        let first = sample_cell("project:continuitydb:stale-index-first", 0.91, 12)?;
        let second = sample_cell("project:continuitydb:stale-index-second", 0.83, 15)?;
        let expected_ids = vec![first.id, second.id];
        let mut kernel = FileKernel::open(&path)?;
        kernel.append_cells_at(vec![first], committed_at)?;
        let stale_checkpoint = fs::read(&index_path)?;
        kernel.append_cells_at(vec![second], committed_at)?;
        fs::write(&index_path, stale_checkpoint)?;
        kernel.index.cells.clear();

        let results = kernel.lookup_cells(CellLookup::default())?;

        assert_eq!(
            results.iter().map(|cell| cell.id).collect::<Vec<_>>(),
            expected_ids
        );
        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rebuilds_incomplete_persistent_index_checkpoint(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-incomplete-persistent-index");
        let index_path = temp_persistent_index_path(&path);
        let committed_at = test_commit_time()?;
        let first = sample_cell("project:continuitydb:incomplete-index-first", 0.91, 12)?;
        let second = sample_cell("project:continuitydb:incomplete-index-second", 0.83, 15)?;
        let expected_ids = vec![first.id, second.id];
        let mut kernel = FileKernel::open(&path)?;
        kernel.append_cells_at(vec![first, second], committed_at)?;

        let mut checkpoint: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&index_path)?)?;
        checkpoint["cells"]
            .as_array_mut()
            .ok_or_else(|| std::io::Error::other("missing persistent cells"))?
            .pop();
        fs::write(&index_path, serde_json::to_vec_pretty(&checkpoint)?)?;
        kernel.index.cells.clear();

        let results = kernel.lookup_cells(CellLookup::default())?;

        assert_eq!(
            results.iter().map(|cell| cell.id).collect::<Vec<_>>(),
            expected_ids
        );
        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rebuilds_duplicate_primary_cell_persistent_index_addresses(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-duplicate-primary-cell-index");
        let index_path = temp_persistent_index_path(&path);
        let committed_at = test_commit_time()?;
        let first = sample_cell(
            "project:continuitydb:duplicate-primary-index-first",
            0.91,
            12,
        )?;
        let second = sample_cell(
            "project:continuitydb:duplicate-primary-index-second",
            0.83,
            15,
        )?;
        let expected_ids = vec![first.id, second.id];
        let mut kernel = FileKernel::open(&path)?;
        kernel.append_cells_at(vec![first, second], committed_at)?;

        let mut checkpoint: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&index_path)?)?;
        let duplicate_address = checkpoint["cells"][0].clone();
        checkpoint["cells"][1] = duplicate_address;
        fs::write(&index_path, serde_json::to_vec_pretty(&checkpoint)?)?;
        kernel.index.cells.clear();

        let results = kernel.lookup_cells(CellLookup::default())?;

        assert_eq!(
            results.iter().map(|cell| cell.id).collect::<Vec<_>>(),
            expected_ids
        );
        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rebuilds_incomplete_secondary_persistent_index_checkpoint(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-incomplete-secondary-index");
        let index_path = temp_persistent_index_path(&path);
        let committed_at = test_commit_time()?;
        let anchor = "project:continuitydb:incomplete-secondary-index";
        let cell = sample_cell(anchor, 0.91, 12)?;
        let expected_id = cell.id;
        let mut kernel = FileKernel::open(&path)?;
        kernel.append_cells_at(vec![cell], committed_at)?;

        let mut checkpoint: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&index_path)?)?;
        checkpoint["semantic_anchors"]
            .as_array_mut()
            .ok_or_else(|| std::io::Error::other("missing semantic anchors"))?
            .pop();
        fs::write(&index_path, serde_json::to_vec_pretty(&checkpoint)?)?;
        kernel.index.anchors.clear();
        kernel.index.cells.clear();

        let results = kernel.lookup_cells(CellLookup {
            semantic_anchor: Some(anchor.to_string()),
            ..CellLookup::default()
        })?;

        assert_eq!(
            results.iter().map(|cell| cell.id).collect::<Vec<_>>(),
            vec![expected_id]
        );
        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rebuilds_corrupt_persistent_index_address_checksum(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-corrupt-index-address");
        let index_path = temp_persistent_index_path(&path);
        let committed_at = test_commit_time()?;
        let cell = sample_cell("project:continuitydb:corrupt-index-address", 0.91, 12)?;
        let expected_id = cell.id;
        let mut kernel = FileKernel::open(&path)?;
        kernel.append_cells_at(vec![cell], committed_at)?;

        let mut checkpoint: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&index_path)?)?;
        checkpoint["cells"][0]["checksum"] =
            serde_json::Value::String("continuitydb-fnv1a64:0000000000000000".to_string());
        fs::write(&index_path, serde_json::to_vec_pretty(&checkpoint)?)?;
        kernel.index.cells.clear();

        let results = kernel.lookup_cells(CellLookup::default())?;

        assert_eq!(
            results.iter().map(|cell| cell.id).collect::<Vec<_>>(),
            vec![expected_id]
        );
        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_health_reports_trusted_persistent_index_checkpoint(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-trusted-index-health");
        let index_path = temp_persistent_index_path(&path);
        {
            let mut kernel = FileKernel::open(&path)?;
            kernel.append_cell(sample_cell("project:continuitydb:trusted-index", 0.91, 12)?)?;
        }

        let reopened = FileKernel::open(&path)?;
        let health = reopened.health();

        assert!(health.persistent_index_checkpoint_present_on_open);
        assert!(health.persistent_index_checkpoint_trusted_on_open);
        assert!(!health.persistent_index_checkpoint_rebuilt_on_open);
        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_health_reports_rebuilt_persistent_index_checkpoint(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-rebuilt-index-health");
        let index_path = temp_persistent_index_path(&path);
        {
            let mut kernel = FileKernel::open(&path)?;
            kernel.append_cell(sample_cell("project:continuitydb:rebuilt-index", 0.91, 12)?)?;
        }
        let mut checkpoint: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&index_path)?)?;
        checkpoint["cells"][0]["checksum"] = serde_json::json!("corrupt");
        fs::write(&index_path, serde_json::to_vec_pretty(&checkpoint)?)?;

        let reopened = FileKernel::open(&path)?;
        let health = reopened.health();

        assert!(health.persistent_index_checkpoint_present_on_open);
        assert!(!health.persistent_index_checkpoint_trusted_on_open);
        assert!(health.persistent_index_checkpoint_rebuilt_on_open);
        assert_eq!(reopened.lookup_cells(CellLookup::default())?.len(), 1);
        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rebuilds_corrupt_secondary_persistent_index_address_checksum(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-corrupt-secondary-index-address");
        let index_path = temp_persistent_index_path(&path);
        let committed_at = test_commit_time()?;
        let anchor = "project:continuitydb:corrupt-secondary-index-address";
        let cell = sample_cell(anchor, 0.91, 12)?;
        let expected_id = cell.id;
        let mut kernel = FileKernel::open(&path)?;
        kernel.append_cells_at(vec![cell], committed_at)?;

        let mut checkpoint: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&index_path)?)?;
        checkpoint["semantic_anchors"][0]["checksum"] =
            serde_json::Value::String("continuitydb-fnv1a64:0000000000000000".to_string());
        fs::write(&index_path, serde_json::to_vec_pretty(&checkpoint)?)?;
        kernel.index.anchors.clear();
        kernel.index.cells.clear();

        let results = kernel.lookup_cells(CellLookup {
            semantic_anchor: Some(anchor.to_string()),
            ..CellLookup::default()
        })?;

        assert_eq!(
            results.iter().map(|cell| cell.id).collect::<Vec<_>>(),
            vec![expected_id]
        );
        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rebuilds_tampered_semantic_anchor_persistent_index_key(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-tampered-anchor-index-key");
        let index_path = temp_persistent_index_path(&path);
        let committed_at = test_commit_time()?;
        let anchor = "project:continuitydb:tampered-anchor-index-key";
        let cell = sample_cell(anchor, 0.91, 12)?;
        let expected_id = cell.id;
        let mut kernel = FileKernel::open(&path)?;
        kernel.append_cells_at(vec![cell], committed_at)?;

        let mut checkpoint: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&index_path)?)?;
        checkpoint["semantic_anchors"][0]["anchor"] =
            serde_json::Value::String("project:continuitydb:wrong-anchor".to_string());
        fs::write(&index_path, serde_json::to_vec_pretty(&checkpoint)?)?;
        kernel.index.anchors.clear();
        kernel.index.cells.clear();

        let results = kernel.lookup_cells(CellLookup {
            semantic_anchor: Some(anchor.to_string()),
            ..CellLookup::default()
        })?;

        assert_eq!(
            results.iter().map(|cell| cell.id).collect::<Vec<_>>(),
            vec![expected_id]
        );
        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_persists_batch_with_explicit_commit_id() -> Result<(), Box<dyn std::error::Error>>
    {
        let path = temp_kernel_path("continuitydb-file-kernel-commit-id");
        let committed_at = test_commit_time()?;
        let commit_id = CommitId::new();
        let first = sample_cell("project:continuitydb:commit-first", 0.91, 12)?;
        let second = sample_cell("project:continuitydb:commit-second", 0.83, 15)?;
        let expected_ids = vec![first.id, second.id];
        {
            let mut kernel = FileKernel::open(&path)?;
            kernel.append_cells_at_with_commit_id(vec![first, second], committed_at, commit_id)?;
        }

        let reopened = FileKernel::open(&path)?;
        let results = reopened.lookup_cells(CellLookup {
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
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_reconstructs_commit_manifest_after_reopen(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-manifest");
        let committed_at = test_commit_time()?;
        let commit_id = CommitId::new();
        let first = sample_cell("project:continuitydb:manifest-first", 0.91, 12)?;
        let second = sample_cell("project:continuitydb:manifest-second", 0.83, 15)?;
        let expected_ids = vec![first.id, second.id];
        {
            let mut kernel = FileKernel::open(&path)?;
            kernel.append_cells_at_with_commit_id(vec![first, second], committed_at, commit_id)?;
        }

        let reopened = FileKernel::open(&path)?;
        let manifest = reopened
            .lookup_commit_manifest(commit_id)?
            .ok_or_else(|| std::io::Error::other("missing manifest"))?;

        assert_eq!(manifest.commit_id, commit_id);
        assert_eq!(manifest.committed_at, committed_at);
        assert_eq!(manifest.cell_ids, expected_ids);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn revision_link_storage_file_kernel_persists_links_across_reopen(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-revision-links-reopen");
        let record = RevisionLinkRecord::new(
            StateCellId::from_u128(1),
            RevisionLinkKind::Supersedes,
            StateCellId::from_u128(2),
            test_commit_time()?,
        );
        {
            let mut kernel = FileKernel::open(&path)?;
            kernel.append_revision_link(record.clone())?;
        }

        let reopened = FileKernel::open(&path)?;

        assert_eq!(
            reopened.list_revision_links(RevisionLinkLookup::default())?,
            vec![record]
        );
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn revision_link_storage_file_kernel_rejects_duplicate_links_without_writing_record(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-revision-links-duplicate");
        let record = RevisionLinkRecord::new(
            StateCellId::from_u128(1),
            RevisionLinkKind::Supersedes,
            StateCellId::from_u128(2),
            test_commit_time()?,
        );
        let mut kernel = FileKernel::open(&path)?;

        kernel.append_revision_link(record.clone())?;
        let result = kernel.append_revision_link(record.clone());

        assert!(matches!(result, Err(KernelError::DuplicateRevisionLink)));
        assert_eq!(
            kernel.list_revision_links(RevisionLinkLookup::default())?,
            vec![record]
        );
        assert_eq!(fs::read_to_string(&path)?.lines().count(), 2);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_duplicate_revision_link_guard_uses_persistent_index_addresses(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-revision-links-duplicate-index");
        let index_path = temp_persistent_index_path(&path);
        let record = RevisionLinkRecord::new(
            StateCellId::from_u128(1),
            RevisionLinkKind::Supersedes,
            StateCellId::from_u128(2),
            test_commit_time()?,
        );
        {
            let mut kernel = FileKernel::open(&path)?;
            kernel.append_revision_link(record.clone())?;
        }
        let mut reopened = FileKernel::open(&path)?;
        reopened.index.revision_links.clear();

        let result = reopened.append_revision_link(record);

        assert!(matches!(result, Err(KernelError::DuplicateRevisionLink)));
        assert_eq!(fs::read_to_string(&path)?.lines().count(), 2);
        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn revision_link_storage_file_kernel_rejects_duplicate_links_on_reopen(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-revision-links-duplicate-reopen");
        let record = RevisionLinkRecord::new(
            StateCellId::from_u128(1),
            RevisionLinkKind::Supersedes,
            StateCellId::from_u128(2),
            test_commit_time()?,
        );
        fs::write(
            &path,
            format!(
                "{}\n{}\n{}\n",
                serde_json::json!({
                    "type": "header",
                    "format": "continuitydb.file_kernel",
                    "version": 1
                }),
                serde_json::json!({
                    "type": "revision_link",
                    "revision_link": record
                }),
                serde_json::json!({
                    "type": "revision_link",
                    "revision_link": record
                })
            ),
        )?;

        let result = FileKernel::open(&path);

        assert!(matches!(result, Err(KernelError::DuplicateRevisionLink)));
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn revision_link_storage_file_kernel_filters_by_source_target_and_kind(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-revision-links-filter");
        let recorded_at = test_commit_time()?;
        let source = StateCellId::from_u128(10);
        let target = StateCellId::from_u128(20);
        let matching =
            RevisionLinkRecord::new(source, RevisionLinkKind::Supersedes, target, recorded_at);
        let different_kind =
            RevisionLinkRecord::new(source, RevisionLinkKind::ConflictsWith, target, recorded_at);
        let different_target = RevisionLinkRecord::new(
            source,
            RevisionLinkKind::Supersedes,
            StateCellId::from_u128(30),
            recorded_at,
        );
        let mut kernel = FileKernel::open(&path)?;
        kernel.append_revision_link(different_kind)?;
        kernel.append_revision_link(matching.clone())?;
        kernel.append_revision_link(different_target)?;

        let results = kernel.list_revision_links(RevisionLinkLookup {
            source: Some(source),
            target: Some(target),
            kind: Some(RevisionLinkKind::Supersedes),
        })?;

        assert_eq!(results, vec![matching]);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_revision_link_source_lookup_uses_persistent_index_addresses(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-revision-link-source-index-lookup");
        let index_path = temp_persistent_index_path(&path);
        let recorded_at = test_commit_time()?;
        let source = StateCellId::from_u128(10);
        let matching = RevisionLinkRecord::new(
            source,
            RevisionLinkKind::Supersedes,
            StateCellId::from_u128(20),
            recorded_at,
        );
        let unrelated = RevisionLinkRecord::new(
            StateCellId::from_u128(30),
            RevisionLinkKind::Supersedes,
            StateCellId::from_u128(40),
            recorded_at,
        );
        {
            let mut kernel = FileKernel::open(&path)?;
            kernel.append_revision_link(matching.clone())?;
            kernel.append_revision_link(unrelated)?;
        }
        let mut reopened = FileKernel::open(&path)?;
        reopened.index.revision_link_sources.clear();
        reopened.index.revision_links.clear();

        let results = reopened.list_revision_links(RevisionLinkLookup {
            source: Some(source),
            ..RevisionLinkLookup::default()
        })?;

        assert_eq!(results, vec![matching]);

        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_revision_link_target_lookup_uses_persistent_index_addresses(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-revision-link-target-index-lookup");
        let index_path = temp_persistent_index_path(&path);
        let recorded_at = test_commit_time()?;
        let target = StateCellId::from_u128(20);
        let matching = RevisionLinkRecord::new(
            StateCellId::from_u128(10),
            RevisionLinkKind::Supersedes,
            target,
            recorded_at,
        );
        let unrelated = RevisionLinkRecord::new(
            StateCellId::from_u128(30),
            RevisionLinkKind::Supersedes,
            StateCellId::from_u128(40),
            recorded_at,
        );
        {
            let mut kernel = FileKernel::open(&path)?;
            kernel.append_revision_link(matching.clone())?;
            kernel.append_revision_link(unrelated)?;
        }
        let mut reopened = FileKernel::open(&path)?;
        reopened.index.revision_link_targets.clear();
        reopened.index.revision_links.clear();

        let results = reopened.list_revision_links(RevisionLinkLookup {
            target: Some(target),
            ..RevisionLinkLookup::default()
        })?;

        assert_eq!(results, vec![matching]);

        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rebuilds_corrupt_revision_link_target_index_address(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-revision-link-corrupt-target-index");
        let index_path = temp_persistent_index_path(&path);
        let recorded_at = test_commit_time()?;
        let target = StateCellId::from_u128(20);
        let record = RevisionLinkRecord::new(
            StateCellId::from_u128(10),
            RevisionLinkKind::Supersedes,
            target,
            recorded_at,
        );
        let mut kernel = FileKernel::open(&path)?;
        kernel.append_revision_link(record.clone())?;

        let mut checkpoint: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&index_path)?)?;
        checkpoint["revision_link_targets"][0]["checksum"] =
            serde_json::Value::String("continuitydb-fnv1a64:0000000000000000".to_string());
        fs::write(&index_path, serde_json::to_vec_pretty(&checkpoint)?)?;
        kernel.index.revision_link_targets.clear();
        kernel.index.revision_links.clear();

        let results = kernel.list_revision_links(RevisionLinkLookup {
            target: Some(target),
            ..RevisionLinkLookup::default()
        })?;

        assert_eq!(results, vec![record]);
        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_revision_link_kind_lookup_uses_persistent_index_addresses(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-revision-link-kind-index-lookup");
        let index_path = temp_persistent_index_path(&path);
        let recorded_at = test_commit_time()?;
        let first = RevisionLinkRecord::new(
            StateCellId::from_u128(10),
            RevisionLinkKind::Supersedes,
            StateCellId::from_u128(20),
            recorded_at,
        );
        let conflict = RevisionLinkRecord::new(
            StateCellId::from_u128(30),
            RevisionLinkKind::ConflictsWith,
            StateCellId::from_u128(40),
            recorded_at,
        );
        let second = RevisionLinkRecord::new(
            StateCellId::from_u128(50),
            RevisionLinkKind::Supersedes,
            StateCellId::from_u128(60),
            recorded_at,
        );
        {
            let mut kernel = FileKernel::open(&path)?;
            kernel.append_revision_link(first.clone())?;
            kernel.append_revision_link(conflict)?;
            kernel.append_revision_link(second.clone())?;
        }
        let mut reopened = FileKernel::open(&path)?;
        reopened.index.revision_link_kinds.clear();
        reopened.index.revision_links.clear();

        let results = reopened.list_revision_links(RevisionLinkLookup {
            kind: Some(RevisionLinkKind::Supersedes),
            ..RevisionLinkLookup::default()
        })?;

        assert_eq!(results, vec![first, second]);

        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_revision_link_listing_uses_persistent_index_addresses(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-revision-link-listing-index-lookup");
        let index_path = temp_persistent_index_path(&path);
        let recorded_at = test_commit_time()?;
        let first = RevisionLinkRecord::new(
            StateCellId::from_u128(10),
            RevisionLinkKind::Supersedes,
            StateCellId::from_u128(20),
            recorded_at,
        );
        let second = RevisionLinkRecord::new(
            StateCellId::from_u128(30),
            RevisionLinkKind::ConflictsWith,
            StateCellId::from_u128(40),
            recorded_at,
        );
        {
            let mut kernel = FileKernel::open(&path)?;
            kernel.append_revision_link(first.clone())?;
            kernel.append_revision_link(second.clone())?;
        }
        let mut reopened = FileKernel::open(&path)?;
        reopened.index.revision_links.clear();

        let results = reopened.list_revision_links(RevisionLinkLookup::default())?;

        assert_eq!(results, vec![first, second]);

        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rebuilds_duplicate_revision_link_persistent_index_addresses(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-revision-link-duplicate-index-address");
        let index_path = temp_persistent_index_path(&path);
        let recorded_at = test_commit_time()?;
        let first = RevisionLinkRecord::new(
            StateCellId::from_u128(10),
            RevisionLinkKind::Supersedes,
            StateCellId::from_u128(20),
            recorded_at,
        );
        let second = RevisionLinkRecord::new(
            StateCellId::from_u128(30),
            RevisionLinkKind::ConflictsWith,
            StateCellId::from_u128(40),
            recorded_at,
        );
        let mut kernel = FileKernel::open(&path)?;
        kernel.append_revision_link(first.clone())?;
        kernel.append_revision_link(second.clone())?;

        let mut checkpoint: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&index_path)?)?;
        let duplicate_address = checkpoint["revision_link_sources"][0].clone();
        checkpoint["revision_link_sources"][1] = duplicate_address;
        fs::write(&index_path, serde_json::to_vec_pretty(&checkpoint)?)?;
        kernel.index.revision_links.clear();

        let results = kernel.list_revision_links(RevisionLinkLookup::default())?;

        assert_eq!(results, vec![first, second]);
        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rebuilds_duplicate_revision_link_target_persistent_index_addresses(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path =
            temp_kernel_path("continuitydb-file-revision-link-duplicate-target-index-address");
        let index_path = temp_persistent_index_path(&path);
        let recorded_at = test_commit_time()?;
        let first_target = StateCellId::from_u128(20);
        let second_target = StateCellId::from_u128(40);
        let first = RevisionLinkRecord::new(
            StateCellId::from_u128(10),
            RevisionLinkKind::Supersedes,
            first_target,
            recorded_at,
        );
        let second = RevisionLinkRecord::new(
            StateCellId::from_u128(30),
            RevisionLinkKind::ConflictsWith,
            second_target,
            recorded_at,
        );
        let mut kernel = FileKernel::open(&path)?;
        kernel.append_revision_link(first)?;
        kernel.append_revision_link(second.clone())?;

        let mut checkpoint: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&index_path)?)?;
        let duplicate_address = checkpoint["revision_link_targets"][0].clone();
        checkpoint["revision_link_targets"][1] = duplicate_address;
        fs::write(&index_path, serde_json::to_vec_pretty(&checkpoint)?)?;
        kernel.index.revision_link_targets.clear();
        kernel.index.revision_links.clear();

        let results = kernel.list_revision_links(RevisionLinkLookup {
            target: Some(second_target),
            ..RevisionLinkLookup::default()
        })?;

        assert_eq!(results, vec![second]);
        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rebuilds_duplicate_revision_link_kind_persistent_index_addresses(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-revision-link-duplicate-kind-index-address");
        let index_path = temp_persistent_index_path(&path);
        let recorded_at = test_commit_time()?;
        let first = RevisionLinkRecord::new(
            StateCellId::from_u128(10),
            RevisionLinkKind::Supersedes,
            StateCellId::from_u128(20),
            recorded_at,
        );
        let second = RevisionLinkRecord::new(
            StateCellId::from_u128(30),
            RevisionLinkKind::ConflictsWith,
            StateCellId::from_u128(40),
            recorded_at,
        );
        let mut kernel = FileKernel::open(&path)?;
        kernel.append_revision_link(first)?;
        kernel.append_revision_link(second.clone())?;

        let mut checkpoint: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&index_path)?)?;
        let duplicate_address = checkpoint["revision_link_kinds"][0].clone();
        checkpoint["revision_link_kinds"][1] = duplicate_address;
        fs::write(&index_path, serde_json::to_vec_pretty(&checkpoint)?)?;
        kernel.index.revision_link_kinds.clear();
        kernel.index.revision_links.clear();

        let results = kernel.list_revision_links(RevisionLinkLookup {
            kind: Some(RevisionLinkKind::ConflictsWith),
            ..RevisionLinkLookup::default()
        })?;

        assert_eq!(results, vec![second]);
        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rebuilds_revision_link_indexes() -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-revision-link-index-rebuild");
        let recorded_at = test_commit_time()?;
        let source = StateCellId::from_u128(10);
        let target = StateCellId::from_u128(20);
        let other_source = StateCellId::from_u128(30);
        let other_target = StateCellId::from_u128(40);
        let same_target = RevisionLinkRecord::new(
            other_source,
            RevisionLinkKind::ConflictsWith,
            target,
            recorded_at,
        );
        let matching =
            RevisionLinkRecord::new(source, RevisionLinkKind::Supersedes, target, recorded_at);
        let same_source = RevisionLinkRecord::new(
            source,
            RevisionLinkKind::Supersedes,
            other_target,
            recorded_at,
        );
        let same_kind = RevisionLinkRecord::new(
            other_source,
            RevisionLinkKind::Supersedes,
            other_target,
            recorded_at,
        );
        {
            let mut kernel = FileKernel::open(&path)?;
            kernel.append_revision_link(same_target.clone())?;
            kernel.append_revision_link(matching.clone())?;
            kernel.append_revision_link(same_source.clone())?;
            kernel.append_revision_link(same_kind.clone())?;
        }

        let reopened = FileKernel::open(&path)?;

        assert_eq!(
            indexed_revision_links(
                &reopened.index,
                &reopened
                    .index
                    .revision_link_sources
                    .get(&source)
                    .cloned()
                    .unwrap_or_default()
            ),
            vec![matching.clone(), same_source.clone()]
        );
        assert_eq!(
            indexed_revision_links(
                &reopened.index,
                &reopened
                    .index
                    .revision_link_targets
                    .get(&target)
                    .cloned()
                    .unwrap_or_default()
            ),
            vec![same_target, matching.clone()]
        );
        assert_eq!(
            indexed_revision_links(
                &reopened.index,
                &reopened
                    .index
                    .revision_link_kinds
                    .get(&RevisionLinkKind::Supersedes)
                    .cloned()
                    .unwrap_or_default()
            ),
            vec![matching.clone(), same_source.clone(), same_kind]
        );
        assert_eq!(
            indexed_revision_links(
                &reopened.index,
                &reopened
                    .index
                    .revision_link_source_kinds
                    .get(&(source, RevisionLinkKind::Supersedes))
                    .cloned()
                    .unwrap_or_default()
            ),
            vec![matching.clone(), same_source]
        );
        assert_eq!(
            indexed_revision_links(
                &reopened.index,
                &reopened
                    .index
                    .revision_link_target_kinds
                    .get(&(target, RevisionLinkKind::Supersedes))
                    .cloned()
                    .unwrap_or_default()
            ),
            vec![matching.clone()]
        );
        assert_eq!(
            indexed_revision_links(
                &reopened.index,
                &reopened
                    .index
                    .revision_link_source_targets
                    .get(&(source, target))
                    .cloned()
                    .unwrap_or_default()
            ),
            vec![matching.clone()]
        );
        assert_eq!(
            indexed_revision_links(
                &reopened.index,
                &reopened
                    .index
                    .revision_link_source_target_kinds
                    .get(&(source, target, RevisionLinkKind::Supersedes))
                    .cloned()
                    .unwrap_or_default()
            ),
            vec![matching.clone()]
        );
        assert_eq!(
            reopened.list_revision_links(RevisionLinkLookup {
                source: Some(source),
                target: Some(target),
                kind: Some(RevisionLinkKind::Supersedes),
            })?,
            vec![matching]
        );
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_updates_revision_link_indexes_after_append(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-revision-link-index-append");
        let source = StateCellId::from_u128(100);
        let target = StateCellId::from_u128(200);
        let record = RevisionLinkRecord::new(
            source,
            RevisionLinkKind::DerivesFrom,
            target,
            test_commit_time()?,
        );
        let mut kernel = FileKernel::open(&path)?;

        kernel.append_revision_link(record.clone())?;

        assert_eq!(
            indexed_revision_links(
                &kernel.index,
                &kernel
                    .index
                    .revision_link_sources
                    .get(&source)
                    .cloned()
                    .unwrap_or_default()
            ),
            vec![record.clone()]
        );
        assert_eq!(
            indexed_revision_links(
                &kernel.index,
                &kernel
                    .index
                    .revision_link_targets
                    .get(&target)
                    .cloned()
                    .unwrap_or_default()
            ),
            vec![record.clone()]
        );
        assert_eq!(
            indexed_revision_links(
                &kernel.index,
                &kernel
                    .index
                    .revision_link_kinds
                    .get(&RevisionLinkKind::DerivesFrom)
                    .cloned()
                    .unwrap_or_default()
            ),
            vec![record.clone()]
        );
        assert_eq!(
            indexed_revision_links(
                &kernel.index,
                &kernel
                    .index
                    .revision_link_source_kinds
                    .get(&(source, RevisionLinkKind::DerivesFrom))
                    .cloned()
                    .unwrap_or_default()
            ),
            vec![record.clone()]
        );
        assert_eq!(
            indexed_revision_links(
                &kernel.index,
                &kernel
                    .index
                    .revision_link_target_kinds
                    .get(&(target, RevisionLinkKind::DerivesFrom))
                    .cloned()
                    .unwrap_or_default()
            ),
            vec![record.clone()]
        );
        assert_eq!(
            indexed_revision_links(
                &kernel.index,
                &kernel
                    .index
                    .revision_link_source_targets
                    .get(&(source, target))
                    .cloned()
                    .unwrap_or_default()
            ),
            vec![record.clone()]
        );
        assert_eq!(
            indexed_revision_links(
                &kernel.index,
                &kernel
                    .index
                    .revision_link_source_target_kinds
                    .get(&(source, target, RevisionLinkKind::DerivesFrom))
                    .cloned()
                    .unwrap_or_default()
            ),
            vec![record.clone()]
        );
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn revision_link_storage_file_kernel_compaction_preserves_links(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-revision-links-compact");
        let record = RevisionLinkRecord::new(
            StateCellId::from_u128(100),
            RevisionLinkKind::DerivesFrom,
            StateCellId::from_u128(200),
            test_commit_time()?,
        );
        let mut kernel = FileKernel::open(&path)?;
        kernel.append_revision_link(record.clone())?;

        kernel.compact()?;
        let reopened = FileKernel::open(&path)?;

        assert_eq!(
            reopened.list_revision_links(RevisionLinkLookup::default())?,
            vec![record]
        );
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_writes_explicit_cell_and_commit_records(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-explicit-commit-record");
        let committed_at = test_commit_time()?;
        let commit_id = CommitId::new();
        let first = sample_cell("project:continuitydb:record-first", 0.91, 12)?;
        let second = sample_cell("project:continuitydb:record-second", 0.83, 15)?;
        let expected_ids = vec![first.id, second.id];

        {
            let mut kernel = FileKernel::open(&path)?;
            kernel.append_cells_at_with_commit_id(vec![first, second], committed_at, commit_id)?;
        }

        let lines = fs::read_to_string(&path)?
            .lines()
            .map(serde_json::from_str::<serde_json::Value>)
            .collect::<Result<Vec<_>, _>>()?;

        assert_eq!(lines.len(), 4);
        assert_eq!(lines[0]["type"], "header");
        assert_eq!(lines[1]["type"], "cell");
        assert_eq!(lines[2]["type"], "cell");
        assert_eq!(lines[3]["type"], "commit");
        assert_eq!(
            lines[3]["manifest"]["commit_id"],
            serde_json::to_value(commit_id)?
        );
        assert_eq!(
            lines[3]["manifest"]["cell_ids"],
            serde_json::to_value(expected_ids)?
        );
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_reopens_explicit_commit_records_as_authoritative_manifest(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-reopen-explicit-commit");
        let committed_at = test_commit_time()?;
        let commit_id = CommitId::new();
        let first = sample_cell("project:continuitydb:explicit-first", 0.91, 12)?;
        let second = sample_cell("project:continuitydb:explicit-second", 0.83, 15)?;
        let expected_ids = vec![first.id, second.id];
        {
            let mut kernel = FileKernel::open(&path)?;
            kernel.append_cells_at_with_commit_id(vec![first, second], committed_at, commit_id)?;
        }

        let reopened = FileKernel::open(&path)?;
        let manifest = reopened
            .lookup_commit_manifest(commit_id)?
            .ok_or_else(|| std::io::Error::other("missing manifest"))?;

        assert_eq!(manifest.commit_id, commit_id);
        assert_eq!(manifest.committed_at, committed_at);
        assert_eq!(manifest.cell_ids, expected_ids);
        assert_eq!(reopened.list_commit_manifests()?, vec![manifest]);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_commit_manifest_lookup_uses_persistent_index_addresses(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-commit-manifest-index-lookup");
        let index_path = temp_persistent_index_path(&path);
        let committed_at = test_commit_time()?;
        let commit_id = CommitId::new();
        let first = sample_cell("project:continuitydb:persistent-manifest-first", 0.91, 12)?;
        let second = sample_cell("project:continuitydb:persistent-manifest-second", 0.83, 15)?;
        let expected_ids = vec![first.id, second.id];
        {
            let mut kernel = FileKernel::open(&path)?;
            kernel.append_cells_at_with_commit_id(vec![first, second], committed_at, commit_id)?;
        }
        let mut reopened = FileKernel::open(&path)?;
        reopened.index.manifests.clear();

        let manifest = reopened
            .lookup_commit_manifest(commit_id)?
            .ok_or_else(|| std::io::Error::other("missing manifest"))?;

        assert_eq!(manifest.commit_id, commit_id);
        assert_eq!(manifest.committed_at, committed_at);
        assert_eq!(manifest.cell_ids, expected_ids);

        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_commit_manifest_listing_uses_persistent_index_addresses(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-commit-manifest-listing-index");
        let index_path = temp_persistent_index_path(&path);
        let first_time = test_commit_time()?;
        let second_time = Utc
            .with_ymd_and_hms(2026, 5, 20, 12, 30, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let first_commit = CommitId::new();
        let second_commit = CommitId::new();
        let first = sample_cell(
            "project:continuitydb:persistent-manifest-list-first",
            0.91,
            12,
        )?;
        let second = sample_cell(
            "project:continuitydb:persistent-manifest-list-second",
            0.83,
            15,
        )?;
        let expected_first_ids = vec![first.id];
        let expected_second_ids = vec![second.id];
        {
            let mut kernel = FileKernel::open(&path)?;
            kernel.append_cells_at_with_commit_id(vec![first], first_time, first_commit)?;
            kernel.append_cells_at_with_commit_id(vec![second], second_time, second_commit)?;
        }
        let mut reopened = FileKernel::open(&path)?;
        reopened.index.manifest_order.clear();
        reopened.index.manifests.clear();

        let manifests = reopened.list_commit_manifests()?;

        assert_eq!(manifests.len(), 2);
        assert_eq!(manifests[0].commit_id, first_commit);
        assert_eq!(manifests[0].committed_at, first_time);
        assert_eq!(manifests[0].cell_ids, expected_first_ids);
        assert_eq!(manifests[1].commit_id, second_commit);
        assert_eq!(manifests[1].committed_at, second_time);
        assert_eq!(manifests[1].cell_ids, expected_second_ids);

        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rebuilds_duplicate_commit_manifest_persistent_index_addresses(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-duplicate-manifest-index");
        let index_path = temp_persistent_index_path(&path);
        let first_time = test_commit_time()?;
        let second_time = Utc
            .with_ymd_and_hms(2026, 5, 20, 12, 30, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let first_commit = CommitId::new();
        let second_commit = CommitId::new();
        let first = sample_cell(
            "project:continuitydb:duplicate-manifest-index-first",
            0.91,
            12,
        )?;
        let second = sample_cell(
            "project:continuitydb:duplicate-manifest-index-second",
            0.83,
            15,
        )?;
        let mut kernel = FileKernel::open(&path)?;
        kernel.append_cells_at_with_commit_id(vec![first], first_time, first_commit)?;
        kernel.append_cells_at_with_commit_id(vec![second], second_time, second_commit)?;

        let mut checkpoint: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&index_path)?)?;
        let duplicate_address = checkpoint["commit_manifests"][0].clone();
        checkpoint["commit_manifests"][1] = duplicate_address;
        fs::write(&index_path, serde_json::to_vec_pretty(&checkpoint)?)?;
        kernel.index.manifest_order.clear();
        kernel.index.manifests.clear();

        let manifests = kernel.list_commit_manifests()?;

        assert_eq!(
            manifests
                .iter()
                .map(|manifest| manifest.commit_id)
                .collect::<Vec<_>>(),
            vec![first_commit, second_commit]
        );
        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_commit_manifest_cursor_listing_uses_persistent_index_addresses(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path =
            temp_kernel_path("continuitydb-file-kernel-commit-manifest-cursor-listing-index");
        let index_path = temp_persistent_index_path(&path);
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
        let second = sample_cell(
            "project:continuitydb:persistent-manifest-cursor-second",
            0.83,
            15,
        )?;
        let expected_second_ids = vec![second.id];
        {
            let mut kernel = FileKernel::open(&path)?;
            kernel.append_cells_at_with_commit_id(
                vec![sample_cell(
                    "project:continuitydb:persistent-manifest-cursor-first",
                    0.91,
                    12,
                )?],
                first_time,
                first_commit,
            )?;
            kernel.append_cells_at_with_commit_id(vec![second], second_time, second_commit)?;
            kernel.append_cells_at_with_commit_id(
                vec![sample_cell(
                    "project:continuitydb:persistent-manifest-cursor-third",
                    0.77,
                    18,
                )?],
                third_time,
                third_commit,
            )?;
        }
        let mut reopened = FileKernel::open(&path)?;
        reopened.index.manifest_order.clear();
        reopened.index.manifests.clear();

        let manifests = reopened.list_commit_manifests_matching(CommitManifestLookup {
            after: Some(first_commit),
            limit: Some(1),
        })?;

        assert_eq!(manifests.len(), 1);
        assert_eq!(manifests[0].commit_id, second_commit);
        assert_eq!(manifests[0].committed_at, second_time);
        assert_eq!(manifests[0].cell_ids, expected_second_ids);

        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rejects_headered_cell_without_commit_record(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-headered-orphan-cell");
        let committed_at = test_commit_time()?;
        let commit_id = CommitId::new();
        let mut cell = sample_cell("project:continuitydb:orphan-cell", 0.91, 12)?;
        cell.system_time = continuitydb_core::SystemTimeRange::open_from(committed_at);
        cell.commit_id = commit_id;
        fs::write(
            &path,
            format!(
                "{}\n{}\n",
                serde_json::json!({
                    "type": "header",
                    "format": "continuitydb.file_kernel",
                    "version": 1
                }),
                serde_json::json!({
                    "type": "cell",
                    "cell": cell
                })
            ),
        )?;

        let result = FileKernel::open(&path);

        assert!(matches!(result, Err(KernelError::StoreCorrupt)));
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rejects_headered_partial_batch_without_commit_record(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-headered-partial-batch");
        let committed_at = test_commit_time()?;
        let commit_id = CommitId::new();
        let mut first = sample_cell("project:continuitydb:partial-first", 0.91, 12)?;
        first.system_time = continuitydb_core::SystemTimeRange::open_from(committed_at);
        first.commit_id = commit_id;
        fs::write(
            &path,
            format!(
                "{}\n{}\n",
                serde_json::json!({
                    "type": "header",
                    "format": "continuitydb.file_kernel",
                    "version": 1
                }),
                serde_json::json!({
                    "type": "cell",
                    "cell": first
                })
            ),
        )?;

        let result = FileKernel::open(&path);

        assert!(matches!(result, Err(KernelError::StoreCorrupt)));
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_still_opens_legacy_raw_cell_logs() -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-legacy-raw-cells");
        let committed_at = test_commit_time()?;
        let commit_id = CommitId::new();
        let mut first = sample_cell("project:continuitydb:legacy-first", 0.91, 12)?;
        let mut second = sample_cell("project:continuitydb:legacy-second", 0.83, 15)?;
        first.system_time = continuitydb_core::SystemTimeRange::open_from(committed_at);
        first.commit_id = commit_id;
        second.system_time = continuitydb_core::SystemTimeRange::open_from(committed_at);
        second.commit_id = commit_id;
        let expected_ids = vec![first.id, second.id];
        fs::write(
            &path,
            format!(
                "{}\n{}\n",
                serde_json::to_string(&first)?,
                serde_json::to_string(&second)?
            ),
        )?;

        let kernel = FileKernel::open(&path)?;
        let manifest = kernel
            .lookup_commit_manifest(commit_id)?
            .ok_or_else(|| std::io::Error::other("missing legacy manifest"))?;

        assert_eq!(manifest.cell_ids, expected_ids);
        assert_eq!(kernel.lookup_cells(CellLookup::default())?.len(), 2);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rejects_commit_record_with_missing_cell(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-missing-commit-cell");
        let committed_at = test_commit_time()?;
        let commit_id = CommitId::new();
        let manifest = continuitydb_core::CommitManifest::new(
            commit_id,
            committed_at,
            vec![StateCellId::new()],
        );
        fs::write(
            &path,
            format!(
                "{}\n",
                serde_json::json!({
                    "type": "commit",
                    "manifest": manifest
                })
            ),
        )?;

        let result = FileKernel::open(&path);

        assert!(matches!(result, Err(KernelError::StoreCorrupt)));
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rejects_duplicate_explicit_commit_records(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-duplicate-explicit-commit");
        let committed_at = test_commit_time()?;
        let commit_id = CommitId::new();
        let mut cell = sample_cell("project:continuitydb:duplicate-explicit", 0.91, 12)?;
        cell.system_time = continuitydb_core::SystemTimeRange::open_from(committed_at);
        cell.commit_id = commit_id;
        let manifest =
            continuitydb_core::CommitManifest::new(commit_id, committed_at, vec![cell.id]);
        fs::write(
            &path,
            format!(
                "{}\n{}\n{}\n",
                serde_json::json!({
                    "type": "cell",
                    "cell": cell
                }),
                serde_json::json!({
                    "type": "commit",
                    "manifest": manifest
                }),
                serde_json::json!({
                    "type": "commit",
                    "manifest": manifest
                })
            ),
        )?;

        let result = FileKernel::open(&path);

        assert!(matches!(result, Err(KernelError::StoreCorrupt)));
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rejects_commit_record_with_mismatched_cell_commit(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-mismatched-cell-commit");
        let committed_at = test_commit_time()?;
        let cell_commit = CommitId::new();
        let manifest_commit = CommitId::new();
        let mut cell = sample_cell("project:continuitydb:mismatched-cell-commit", 0.91, 12)?;
        cell.system_time = continuitydb_core::SystemTimeRange::open_from(committed_at);
        cell.commit_id = cell_commit;
        let manifest =
            continuitydb_core::CommitManifest::new(manifest_commit, committed_at, vec![cell.id]);
        fs::write(
            &path,
            format!(
                "{}\n{}\n",
                serde_json::json!({
                    "type": "cell",
                    "cell": cell
                }),
                serde_json::json!({
                    "type": "commit",
                    "manifest": manifest
                })
            ),
        )?;

        let result = FileKernel::open(&path);

        assert!(matches!(result, Err(KernelError::StoreCorrupt)));
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_open_writes_header_for_new_empty_file() -> Result<(), Box<dyn std::error::Error>>
    {
        let path = temp_kernel_path("continuitydb-file-kernel-header-new");

        let kernel = FileKernel::open(&path)?;

        assert_eq!(kernel.path(), path.as_path());
        let lines = fs::read_to_string(&path)?
            .lines()
            .map(serde_json::from_str::<serde_json::Value>)
            .collect::<Result<Vec<_>, _>>()?;
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0]["type"], "header");
        assert_eq!(lines[0]["format"], "continuitydb.file_kernel");
        assert_eq!(lines[0]["version"], 1);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_appends_records_after_header() -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-header-before-records");
        let committed_at = test_commit_time()?;
        let commit_id = CommitId::new();
        let cell = sample_cell("project:continuitydb:header-record-order", 0.91, 12)?;
        {
            let mut kernel = FileKernel::open(&path)?;
            kernel.append_cell_at_with_commit_id(cell, committed_at, commit_id)?;
        }

        let lines = fs::read_to_string(&path)?
            .lines()
            .map(serde_json::from_str::<serde_json::Value>)
            .collect::<Result<Vec<_>, _>>()?;
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0]["type"], "header");
        assert_eq!(lines[1]["type"], "cell");
        assert_eq!(lines[2]["type"], "commit");
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rejects_unsupported_header_version() -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-unsupported-header");
        fs::write(
            &path,
            format!(
                "{}\n",
                serde_json::json!({
                    "type": "header",
                    "format": "continuitydb.file_kernel",
                    "version": 999
                })
            ),
        )?;

        let result = FileKernel::open(&path);

        assert!(matches!(
            result,
            Err(KernelError::StoreCorruptRecord { line: 1 })
        ));
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rejects_header_after_data_record() -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-late-header");
        let committed_at = test_commit_time()?;
        let commit_id = CommitId::new();
        let mut cell = sample_cell("project:continuitydb:late-header", 0.91, 12)?;
        cell.system_time = continuitydb_core::SystemTimeRange::open_from(committed_at);
        cell.commit_id = commit_id;
        fs::write(
            &path,
            format!(
                "{}\n{}\n",
                serde_json::json!({
                    "type": "cell",
                    "cell": cell
                }),
                serde_json::json!({
                    "type": "header",
                    "format": "continuitydb.file_kernel",
                    "version": 1
                })
            ),
        )?;

        let result = FileKernel::open(&path);

        assert!(matches!(
            result,
            Err(KernelError::StoreCorruptRecord { line: 2 })
        ));
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_writes_checksums_for_cell_and_commit_records(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-checksummed-records");
        let committed_at = test_commit_time()?;
        let commit_id = CommitId::new();
        let cell = sample_cell("project:continuitydb:checksummed", 0.91, 12)?;
        {
            let mut kernel = FileKernel::open(&path)?;
            kernel.append_cell_at_with_commit_id(cell, committed_at, commit_id)?;
        }

        let lines = fs::read_to_string(&path)?
            .lines()
            .map(serde_json::from_str::<serde_json::Value>)
            .collect::<Result<Vec<_>, _>>()?;

        assert_eq!(lines[1]["type"], "cell");
        assert!(lines[1]["checksum"]
            .as_str()
            .ok_or_else(|| std::io::Error::other("missing cell checksum"))?
            .starts_with("continuitydb-fnv1a64:"));
        assert_eq!(lines[2]["type"], "commit");
        assert!(lines[2]["checksum"]
            .as_str()
            .ok_or_else(|| std::io::Error::other("missing commit checksum"))?
            .starts_with("continuitydb-fnv1a64:"));
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rejects_tampered_cell_checksum() -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-tampered-cell-checksum");
        let committed_at = test_commit_time()?;
        let commit_id = CommitId::new();
        let cell = sample_cell("project:continuitydb:tampered-cell", 0.91, 12)?;
        {
            let mut kernel = FileKernel::open(&path)?;
            kernel.append_cell_at_with_commit_id(cell, committed_at, commit_id)?;
        }
        let mut records = fs::read_to_string(&path)?
            .lines()
            .map(serde_json::from_str::<serde_json::Value>)
            .collect::<Result<Vec<_>, _>>()?;
        records[1]["cell"]["payload"] = serde_json::json!({
            "Text": "tampered payload"
        });
        fs::write(
            &path,
            records
                .into_iter()
                .map(|record| record.to_string())
                .collect::<Vec<_>>()
                .join("\n")
                + "\n",
        )?;

        let result = FileKernel::open(&path);

        assert!(matches!(
            result,
            Err(KernelError::StoreCorruptRecord { line: 2 })
        ));
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rejects_tampered_commit_checksum() -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-tampered-commit-checksum");
        let committed_at = test_commit_time()?;
        let commit_id = CommitId::new();
        let cell = sample_cell("project:continuitydb:tampered-commit", 0.91, 12)?;
        {
            let mut kernel = FileKernel::open(&path)?;
            kernel.append_cell_at_with_commit_id(cell, committed_at, commit_id)?;
        }
        let mut records = fs::read_to_string(&path)?
            .lines()
            .map(serde_json::from_str::<serde_json::Value>)
            .collect::<Result<Vec<_>, _>>()?;
        records[2]["manifest"]["cell_ids"] = serde_json::json!([]);
        fs::write(
            &path,
            records
                .into_iter()
                .map(|record| record.to_string())
                .collect::<Vec<_>>()
                .join("\n")
                + "\n",
        )?;

        let result = FileKernel::open(&path);

        assert!(matches!(
            result,
            Err(KernelError::StoreCorruptRecord { line: 3 })
        ));
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_still_opens_checksum_free_envelope_records(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-checksum-free-envelope");
        let committed_at = test_commit_time()?;
        let commit_id = CommitId::new();
        let mut cell = sample_cell("project:continuitydb:checksum-free", 0.91, 12)?;
        cell.system_time = continuitydb_core::SystemTimeRange::open_from(committed_at);
        cell.commit_id = commit_id;
        let manifest =
            continuitydb_core::CommitManifest::new(commit_id, committed_at, vec![cell.id]);
        fs::write(
            &path,
            format!(
                "{}\n{}\n{}\n",
                serde_json::json!({
                    "type": "header",
                    "format": "continuitydb.file_kernel",
                    "version": 1
                }),
                serde_json::json!({
                    "type": "cell",
                    "cell": cell
                }),
                serde_json::json!({
                    "type": "commit",
                    "manifest": manifest
                })
            ),
        )?;

        let kernel = FileKernel::open(&path)?;

        assert_eq!(kernel.lookup_cells(CellLookup::default())?.len(), 1);
        assert!(kernel.lookup_commit_manifest(commit_id)?.is_some());
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_compacts_legacy_raw_log_to_canonical_records(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-compact-legacy");
        let committed_at = test_commit_time()?;
        let commit_id = CommitId::new();
        let mut first = sample_cell("project:continuitydb:compact-first", 0.91, 12)?;
        let mut second = sample_cell("project:continuitydb:compact-second", 0.83, 15)?;
        first.system_time = continuitydb_core::SystemTimeRange::open_from(committed_at);
        first.commit_id = commit_id;
        second.system_time = continuitydb_core::SystemTimeRange::open_from(committed_at);
        second.commit_id = commit_id;
        let expected_ids = vec![first.id, second.id];
        fs::write(
            &path,
            format!(
                "{}\n{}\n",
                serde_json::to_string(&first)?,
                serde_json::to_string(&second)?
            ),
        )?;

        let mut kernel = FileKernel::open(&path)?;
        kernel.compact()?;

        let records = fs::read_to_string(&path)?
            .lines()
            .map(serde_json::from_str::<serde_json::Value>)
            .collect::<Result<Vec<_>, _>>()?;
        assert_eq!(records.len(), 4);
        assert_eq!(records[0]["type"], "header");
        assert_eq!(records[1]["type"], "cell");
        assert!(records[1]["checksum"].as_str().is_some());
        assert_eq!(records[2]["type"], "cell");
        assert!(records[2]["checksum"].as_str().is_some());
        assert_eq!(records[3]["type"], "commit");
        assert!(records[3]["checksum"].as_str().is_some());
        assert_eq!(
            records[3]["manifest"]["cell_ids"],
            serde_json::to_value(expected_ids)?
        );
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_compaction_preserves_lookup_and_manifest_listing(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-compact-preserve");
        let first_time = test_commit_time()?;
        let second_time = Utc
            .with_ymd_and_hms(2026, 5, 20, 12, 30, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let first_commit = CommitId::new();
        let second_commit = CommitId::new();
        let first = sample_cell("project:continuitydb:compact-list-first", 0.91, 12)?;
        let second = sample_cell("project:continuitydb:compact-list-second", 0.83, 15)?;
        let second_id = second.id;
        {
            let mut kernel = FileKernel::open(&path)?;
            kernel.append_cells_at_with_commit_id(vec![first], first_time, first_commit)?;
            kernel.append_cells_at_with_commit_id(vec![second], second_time, second_commit)?;
            kernel.compact()?;
        }

        let reopened = FileKernel::open(&path)?;
        let manifests = reopened.list_commit_manifests()?;
        let second_lookup = reopened.lookup_cells(CellLookup {
            cell_id: Some(second_id),
            ..CellLookup::default()
        })?;

        assert_eq!(
            manifests
                .iter()
                .map(|manifest| manifest.commit_id)
                .collect::<Vec<_>>(),
            vec![first_commit, second_commit]
        );
        assert_eq!(second_lookup.len(), 1);
        assert_eq!(second_lookup[0].id, second_id);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_compaction_uses_persistent_index_records(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-compact-persistent-index-records");
        let index_path = temp_persistent_index_path(&path);
        let committed_at = test_commit_time()?;
        let commit_id = CommitId::new();
        let first = sample_cell("project:continuitydb:compact-index-first", 0.91, 12)?;
        let second = sample_cell("project:continuitydb:compact-index-second", 0.83, 15)?;
        let first_id = first.id;
        let second_id = second.id;
        let revision_link = RevisionLinkRecord::new(
            first_id,
            RevisionLinkKind::Supersedes,
            second_id,
            committed_at,
        );
        {
            let mut kernel = FileKernel::open(&path)?;
            kernel.append_cells_at_with_commit_id(vec![first, second], committed_at, commit_id)?;
            kernel.append_revision_link(revision_link.clone())?;
        }
        let mut reopened = FileKernel::open(&path)?;
        reopened.index.cells.clear();
        reopened.index.manifest_order.clear();
        reopened.index.manifests.clear();
        reopened.index.revision_links.clear();

        reopened.compact()?;
        let compacted = FileKernel::open(&path)?;
        let status = compacted.status()?;
        let manifests = compacted.list_commit_manifests()?;
        let revision_links = compacted.list_revision_links(RevisionLinkLookup::default())?;

        assert_eq!(status.cell_count, 2);
        assert_eq!(status.commit_count, 1);
        assert_eq!(status.revision_link_count, 1);
        assert_eq!(manifests.len(), 1);
        assert_eq!(manifests[0].commit_id, commit_id);
        assert_eq!(manifests[0].cell_ids, vec![first_id, second_id]);
        assert_eq!(revision_links, vec![revision_link]);

        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_compaction_preserves_cursor_manifest_listing(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-compact-cursor");
        let first_time = test_commit_time()?;
        let second_time = Utc
            .with_ymd_and_hms(2026, 5, 20, 12, 30, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let first_commit = CommitId::new();
        let second_commit = CommitId::new();
        {
            let mut kernel = FileKernel::open(&path)?;
            kernel.append_cells_at_with_commit_id(
                vec![sample_cell(
                    "project:continuitydb:compact-cursor-first",
                    0.91,
                    12,
                )?],
                first_time,
                first_commit,
            )?;
            kernel.append_cells_at_with_commit_id(
                vec![sample_cell(
                    "project:continuitydb:compact-cursor-second",
                    0.83,
                    15,
                )?],
                second_time,
                second_commit,
            )?;
            kernel.compact()?;
        }

        let reopened = FileKernel::open(&path)?;
        let manifests = reopened.list_commit_manifests_matching(CommitManifestLookup {
            after: Some(first_commit),
            limit: Some(1),
        })?;

        assert_eq!(manifests.len(), 1);
        assert_eq!(manifests[0].commit_id, second_commit);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rejects_duplicate_commit_id_without_writing_records(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-duplicate-manifest");
        let committed_at = test_commit_time()?;
        let commit_id = CommitId::new();
        let first = sample_cell("project:continuitydb:manifest-first", 0.91, 12)?;
        let second = sample_cell("project:continuitydb:manifest-second", 0.83, 15)?;
        {
            let mut kernel = FileKernel::open(&path)?;
            kernel.append_cells_at_with_commit_id(vec![first.clone()], committed_at, commit_id)?;
            let result =
                kernel.append_cells_at_with_commit_id(vec![second], committed_at, commit_id);
            assert!(matches!(result, Err(KernelError::DuplicateCommit)));
        }

        let reopened = FileKernel::open(&path)?;
        let stored = reopened.lookup_cells(CellLookup::default())?;
        assert_eq!(
            stored.iter().map(|cell| cell.id).collect::<Vec<_>>(),
            vec![first.id]
        );
        assert_eq!(
            reopened
                .lookup_commit_manifest(commit_id)?
                .ok_or_else(|| std::io::Error::other("missing manifest"))?
                .cell_ids,
            vec![first.id]
        );
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_duplicate_commit_guard_uses_persistent_index_addresses(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-duplicate-commit-persistent-index");
        let index_path = temp_persistent_index_path(&path);
        let committed_at = test_commit_time()?;
        let commit_id = CommitId::new();
        let first = sample_cell(
            "project:continuitydb:duplicate-commit-index-first",
            0.91,
            12,
        )?;
        let second = sample_cell(
            "project:continuitydb:duplicate-commit-index-second",
            0.83,
            15,
        )?;
        {
            let mut kernel = FileKernel::open(&path)?;
            kernel.append_cells_at_with_commit_id(vec![first], committed_at, commit_id)?;
        }
        let mut reopened = FileKernel::open(&path)?;
        reopened.index.manifest_order.clear();
        reopened.index.manifests.clear();

        let result = reopened.append_cells_at_with_commit_id(vec![second], committed_at, commit_id);

        assert!(matches!(result, Err(KernelError::DuplicateCommit)));
        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_reconstructs_commit_manifest_listing_after_reopen(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-manifest-list");
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
        {
            let mut kernel = FileKernel::open(&path)?;
            kernel.append_cells_at_with_commit_id(vec![first], first_time, first_commit)?;
            kernel.append_cells_at_with_commit_id(vec![second], second_time, second_commit)?;
        }

        let reopened = FileKernel::open(&path)?;
        let manifests = reopened.list_commit_manifests()?;

        assert_eq!(manifests.len(), 2);
        assert_eq!(manifests[0].commit_id, first_commit);
        assert_eq!(manifests[0].committed_at, first_time);
        assert_eq!(manifests[0].cell_ids, expected_first_ids);
        assert_eq!(manifests[1].commit_id, second_commit);
        assert_eq!(manifests[1].committed_at, second_time);
        assert_eq!(manifests[1].cell_ids, expected_second_ids);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_reconstructs_cursor_commit_manifest_listing_after_reopen(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-manifest-cursor-list");
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
        {
            let mut kernel = FileKernel::open(&path)?;
            kernel.append_cells_at_with_commit_id(
                vec![sample_cell(
                    "project:continuitydb:file-cursor-first",
                    0.91,
                    12,
                )?],
                first_time,
                first_commit,
            )?;
            kernel.append_cells_at_with_commit_id(
                vec![sample_cell(
                    "project:continuitydb:file-cursor-second",
                    0.83,
                    15,
                )?],
                second_time,
                second_commit,
            )?;
            kernel.append_cells_at_with_commit_id(
                vec![sample_cell(
                    "project:continuitydb:file-cursor-third",
                    0.77,
                    18,
                )?],
                third_time,
                third_commit,
            )?;
        }

        let reopened = FileKernel::open(&path)?;
        let manifests = reopened.list_commit_manifests_matching(CommitManifestLookup {
            after: Some(first_commit),
            limit: Some(1),
        })?;

        assert_eq!(manifests.len(), 1);
        assert_eq!(manifests[0].commit_id, second_commit);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rejects_duplicate_ids_inside_batch_without_writing_records(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-batch-duplicate");
        let committed_at = test_commit_time()?;
        let cell = sample_cell("project:continuitydb:batch-duplicate", 0.91, 12)?;
        {
            let mut kernel = FileKernel::open(&path)?;
            let result = kernel.append_cells_at(vec![cell.clone(), cell], committed_at);
            assert!(matches!(result, Err(KernelError::DuplicateCell)));
        }

        let reopened = FileKernel::open(&path)?;
        assert!(reopened.lookup_cells(CellLookup::default())?.is_empty());
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rejects_existing_ids_inside_batch_without_writing_records(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-batch-existing");
        let committed_at = test_commit_time()?;
        let stored = sample_cell("project:continuitydb:stored", 0.91, 12)?;
        let duplicate = stored.clone();
        let fresh = sample_cell("project:continuitydb:fresh", 0.83, 15)?;
        {
            let mut kernel = FileKernel::open(&path)?;
            kernel.append_cell_at(stored.clone(), committed_at)?;
            let result = kernel.append_cells_at(vec![duplicate, fresh], committed_at);
            assert!(matches!(result, Err(KernelError::DuplicateCell)));
        }

        let reopened = FileKernel::open(&path)?;
        let results = reopened.lookup_cells(CellLookup::default())?;

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, stored.id);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_open_rejects_duplicate_ids_in_log() -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-duplicate-log");
        let cell = sample_cell("project:continuitydb:duplicate-log", 0.91, 12)?;
        let encoded = serde_json::to_string(&cell)?;
        fs::write(&path, format!("{encoded}\n{encoded}\n"))?;

        let result = FileKernel::open(&path);

        assert!(matches!(result, Err(KernelError::DuplicateCell)));
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rejects_corrupt_jsonl() -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-corrupt");
        fs::write(&path, "{not valid json}\n")?;

        let result = FileKernel::open(&path);

        assert!(matches!(
            result,
            Err(KernelError::StoreCorruptRecord { line: 1 })
        ));
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_reports_corrupt_jsonl_line_number() -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-corrupt-line");
        let cell = sample_cell("project:continuitydb:corrupt-line", 0.91, 12)?;
        fs::write(
            &path,
            format!("{}\n{{not valid json}}\n", serde_json::to_string(&cell)?),
        )?;

        let result = FileKernel::open(&path);

        assert!(matches!(
            result,
            Err(KernelError::StoreCorruptRecord { line: 2 })
        ));
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_reports_unsupported_header_line_number() -> Result<(), Box<dyn std::error::Error>>
    {
        let path = temp_kernel_path("continuitydb-file-kernel-unsupported-header-line");
        fs::write(
            &path,
            format!(
                "{}\n",
                serde_json::json!({
                    "type": "header",
                    "format": "continuitydb.file_kernel",
                    "version": 999
                })
            ),
        )?;

        let result = FileKernel::open(&path);

        assert!(matches!(
            result,
            Err(KernelError::StoreCorruptRecord { line: 1 })
        ));
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_reports_checksum_failure_line_number() -> Result<(), Box<dyn std::error::Error>>
    {
        let path = temp_kernel_path("continuitydb-file-kernel-checksum-line");
        {
            let mut kernel = FileKernel::open(&path)?;
            append_committed(
                &mut kernel,
                sample_cell("project:continuitydb:checksum-line", 0.91, 12)?,
            )?;
        }
        let tampered = fs::read_to_string(&path)?.replace(
            "project:continuitydb:checksum-line",
            "project:continuitydb:checksum-line-tampered",
        );
        fs::write(&path, tampered)?;

        let result = FileKernel::open(&path);

        assert!(matches!(
            result,
            Err(KernelError::StoreCorruptRecord { line: 2 })
        ));
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_open_creates_missing_parent_directories(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let root = std::env::temp_dir().join(format!(
            "continuitydb-file-kernel-nested-{:?}",
            StateCellId::new()
        ));
        let path = root.join("db").join("cells.jsonl");
        let parent = path
            .parent()
            .ok_or_else(|| std::io::Error::other("missing parent"))?;
        assert!(!parent.exists());

        let kernel = FileKernel::open(&path)?;

        assert_eq!(kernel.path(), path.as_path());
        assert!(parent.exists());
        assert!(path.exists());
        fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn file_kernel_filters_by_cell_id() -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-cell-id");
        let first = sample_cell("project:continuitydb:first", 0.91, 12)?;
        let mut second = sample_cell("project:continuitydb:second", 0.83, 15)?;
        {
            let mut kernel = FileKernel::open(&path)?;
            append_committed(&mut kernel, first)?;
            second = append_committed(&mut kernel, second)?;
        }

        let reopened = FileKernel::open(&path)?;
        let results = reopened.lookup_cells(CellLookup {
            cell_id: Some(second.id),
            ..CellLookup::default()
        })?;

        assert_eq!(results, vec![second]);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_filters_by_activation_state() -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-activation");
        let mut active = sample_cell("project:continuitydb:active", 0.91, 12)?;
        active.activation = ActivationState::Active;
        let mut frontier = sample_cell("project:continuitydb:frontier", 0.83, 15)?;
        frontier.activation = ActivationState::Frontier;
        {
            let mut kernel = FileKernel::open(&path)?;
            append_committed(&mut kernel, active)?;
            frontier = append_committed(&mut kernel, frontier)?;
        }

        let reopened = FileKernel::open(&path)?;
        let results = reopened.lookup_cells(CellLookup {
            activation: Some(ActivationState::Frontier),
            ..CellLookup::default()
        })?;

        assert_eq!(results, vec![frontier]);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rebuilds_activation_index() -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-activation-index-rebuild");
        let mut active = sample_cell("project:continuitydb:index-active", 0.91, 12)?;
        active.activation = ActivationState::Active;
        let mut frontier = sample_cell("project:continuitydb:index-frontier-activation", 0.83, 15)?;
        frontier.activation = ActivationState::Frontier;
        {
            let mut kernel = FileKernel::open(&path)?;
            append_committed(&mut kernel, active)?;
            frontier = append_committed(&mut kernel, frontier)?;
        }

        let reopened = FileKernel::open(&path)?;
        let positions = reopened
            .index
            .activations
            .get(&ActivationState::Frontier)
            .cloned()
            .unwrap_or_default();
        let indexed = positions
            .iter()
            .map(|position| reopened.index.cells[*position].clone())
            .collect::<Vec<_>>();

        assert_eq!(indexed, vec![frontier]);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_updates_activation_index_after_append() -> Result<(), Box<dyn std::error::Error>>
    {
        let path = temp_kernel_path("continuitydb-file-kernel-activation-index-append");
        let mut frontier = sample_cell(
            "project:continuitydb:index-append-frontier-activation",
            0.83,
            15,
        )?;
        frontier.activation = ActivationState::Frontier;
        let mut kernel = FileKernel::open(&path)?;

        frontier = append_committed(&mut kernel, frontier)?;
        let positions = kernel
            .index
            .activations
            .get(&ActivationState::Frontier)
            .cloned()
            .unwrap_or_default();
        let indexed = positions
            .iter()
            .map(|position| kernel.index.cells[*position].clone())
            .collect::<Vec<_>>();

        assert_eq!(indexed, vec![frontier]);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_filters_by_answerability_question() -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-answerability");
        let mut status = sample_cell("project:continuitydb:status", 0.91, 12)?;
        status.answerability = Answerability::new(vec!["what is status?".to_string()])?;
        let mut frontier = sample_cell("project:continuitydb:frontier", 0.83, 15)?;
        frontier.answerability = Answerability::new(vec!["what is frontier?".to_string()])?;
        {
            let mut kernel = FileKernel::open(&path)?;
            append_committed(&mut kernel, status)?;
            frontier = append_committed(&mut kernel, frontier)?;
        }

        let reopened = FileKernel::open(&path)?;
        let results = reopened.lookup_cells(CellLookup {
            answerability_question: Some("what is frontier?".to_string()),
            ..CellLookup::default()
        })?;

        assert_eq!(results, vec![frontier]);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_filters_by_selection_reason() -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-selection-reason");
        let routine = sample_cell("project:continuitydb:selection-reason-routine", 0.91, 12)?;
        let mut salient = sample_cell("project:continuitydb:selection-reason-attention", 0.83, 15)?;
        salient.set_attention(AttentionSignal::new(0.8, 0.7, 0.9, 0.6)?);
        {
            let mut kernel = FileKernel::open(&path)?;
            append_committed(&mut kernel, routine)?;
            salient = append_committed(&mut kernel, salient)?;
        }

        let reopened = FileKernel::open(&path)?;
        let results = reopened.lookup_cells(CellLookup {
            selection_reason: Some(ContextPacketSelectionReason::AttentionSignal),
            ..CellLookup::default()
        })?;

        assert_eq!(results, vec![salient]);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_filters_by_trajectory_memory_confidence_and_strategy(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-trajectory-memory");
        let mut reusable =
            sample_cell("project:continuitydb:trajectory-memory-reusable", 0.91, 12)?;
        reusable.set_trajectory_memory(TrajectoryMemory::new(
            "reuse failed release upload trajectory",
            "identified package artifact and release command",
            "upload targeted a missing GitHub release",
            "target/alpha-workflow/release-upload-failure.json",
            0.88,
            "verify the release target before trusting upload state",
            vec!["release upload workflow".to_string()],
            vec!["matching successful upload report exists".to_string()],
            ContextPacketStrategy::FalsificationBrief,
        )?);
        let mut weak = sample_cell("project:continuitydb:trajectory-memory-weak", 0.83, 15)?;
        weak.set_trajectory_memory(TrajectoryMemory::new(
            "retry upload from stale checkout",
            "found release workflow",
            "failure was not reproducible",
            "target/weak-trace.json",
            0.4,
            "weak lesson should not cross confidence threshold",
            vec!["release upload workflow".to_string()],
            vec!["fresh run contradicts it".to_string()],
            ContextPacketStrategy::FalsificationBrief,
        )?);
        let mut wrong_shape = sample_cell(
            "project:continuitydb:trajectory-memory-wrong-shape",
            0.83,
            15,
        )?;
        wrong_shape.set_trajectory_memory(TrajectoryMemory::new(
            "look for missing release evidence",
            "found partial logs",
            "trace lacks upload outcome",
            "target/scavenge-trace.json",
            0.91,
            "scavenge retained upload evidence first",
            vec!["release upload workflow".to_string()],
            vec!["complete upload report exists".to_string()],
            ContextPacketStrategy::ScavengingBrief,
        )?);
        {
            let mut kernel = FileKernel::open(&path)?;
            reusable = append_committed(&mut kernel, reusable)?;
            append_committed(&mut kernel, weak)?;
            append_committed(&mut kernel, wrong_shape)?;
        }

        let reopened = FileKernel::open(&path)?;
        let results = reopened.lookup_cells(CellLookup {
            trajectory_memory_strategy: Some(ContextPacketStrategy::FalsificationBrief),
            minimum_trajectory_memory_confidence: Some(Confidence::new(0.8)?),
            ..CellLookup::default()
        })?;

        assert_eq!(results, vec![reusable]);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_trajectory_memory_lookup_uses_persistent_index_addresses(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-trajectory-memory-index-lookup");
        let index_path = temp_persistent_index_path(&path);
        let expected = {
            let mut kernel = FileKernel::open(&path)?;
            append_committed(
                &mut kernel,
                sample_cell(
                    "project:continuitydb:persistent-trajectory-memory-unrelated",
                    0.91,
                    12,
                )?,
            )?;
            let mut weak = sample_cell(
                "project:continuitydb:persistent-trajectory-memory-weak",
                0.83,
                15,
            )?;
            weak.set_trajectory_memory(TrajectoryMemory::new(
                "retry upload from stale checkout",
                "found release workflow",
                "failure was not reproduced",
                "target/weak-trace.json",
                0.42,
                "weak lesson should stay below the reuse threshold",
                vec!["release upload workflow".to_string()],
                vec!["fresh run contradicts it".to_string()],
                ContextPacketStrategy::FalsificationBrief,
            )?);
            let mut reusable = sample_cell(
                "project:continuitydb:persistent-trajectory-memory-high",
                0.83,
                15,
            )?;
            reusable.set_trajectory_memory(TrajectoryMemory::new(
                "reuse failed release upload trajectory",
                "identified package artifact and upload command",
                "upload targeted a missing GitHub release",
                "target/alpha-workflow/release-upload-failure.json",
                0.88,
                "verify release target before trusting upload state",
                vec!["release upload workflow".to_string()],
                vec!["matching successful upload report exists".to_string()],
                ContextPacketStrategy::FalsificationBrief,
            )?);
            append_committed(&mut kernel, weak)?;
            append_committed(&mut kernel, reusable)?
        };
        let checkpoint: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&index_path)?)?;
        assert_eq!(
            checkpoint["trajectory_memory_strategy_address_count"],
            serde_json::json!(2)
        );
        assert_eq!(
            checkpoint["trajectory_memory_confidence_address_count"],
            serde_json::json!(2)
        );
        let mut reopened = FileKernel::open(&path)?;
        reopened.index.cells.clear();

        let cells = reopened.lookup_cells(CellLookup {
            trajectory_memory_strategy: Some(ContextPacketStrategy::FalsificationBrief),
            minimum_trajectory_memory_confidence: Some(Confidence::new(0.8)?),
            ..CellLookup::default()
        })?;

        assert_eq!(vec![expected], cells);

        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_lookup_plan_indexes_trajectory_memory_constraints(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-trajectory-memory-lookup-plan");
        let mut kernel = FileKernel::open(&path)?;
        append_committed(
            &mut kernel,
            sample_cell(
                "project:continuitydb:trajectory-memory-plan-routine",
                0.91,
                12,
            )?,
        )?;
        let mut reusable = sample_cell(
            "project:continuitydb:trajectory-memory-plan-reusable",
            0.83,
            15,
        )?;
        reusable.set_trajectory_memory(TrajectoryMemory::new(
            "reuse failed release upload trajectory",
            "identified package artifact and upload command",
            "upload targeted a missing GitHub release",
            "target/alpha-workflow/release-upload-failure.json",
            0.88,
            "verify release target before trusting upload state",
            vec!["release upload workflow".to_string()],
            vec!["matching successful upload report exists".to_string()],
            ContextPacketStrategy::FalsificationBrief,
        )?);
        append_committed(&mut kernel, reusable)?;

        let plan = kernel.lookup_plan(&CellLookup {
            trajectory_memory_strategy: Some(ContextPacketStrategy::FalsificationBrief),
            minimum_trajectory_memory_confidence: Some(Confidence::new(0.8)?),
            ..CellLookup::default()
        });

        assert_eq!(
            plan.indexed_constraints,
            vec![
                "trajectory_memory_strategy",
                "minimum_trajectory_memory_confidence"
            ]
        );
        assert_eq!(plan.residual_exact_constraints, Vec::<&'static str>::new());
        assert_eq!(plan.candidate_count, 1);
        assert!(!plan.full_scan);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_filters_by_context_gap_kind_and_priority(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-context-gap");
        let routine = sample_cell("project:continuitydb:context-gap-routine", 0.91, 12)?;
        let mut low_priority =
            sample_cell("project:continuitydb:context-gap-low-priority", 0.83, 15)?;
        low_priority.add_context_gap(ContextGap::new(
            ContextGapKind::MissingEvidence,
            "which artifact proves the claim?",
            "low-priority gap should not cross the threshold",
            0.4,
        )?);
        let mut high_priority =
            sample_cell("project:continuitydb:context-gap-high-priority", 0.83, 15)?;
        high_priority.add_context_gap(ContextGap::new(
            ContextGapKind::MissingEvidence,
            "which retained artifact proves the live run?",
            "high-priority missing evidence should remain retrievable",
            0.9,
        )?);
        {
            let mut kernel = FileKernel::open(&path)?;
            append_committed(&mut kernel, routine)?;
            append_committed(&mut kernel, low_priority)?;
            high_priority = append_committed(&mut kernel, high_priority)?;
        }

        let reopened = FileKernel::open(&path)?;
        let results = reopened.lookup_cells(CellLookup {
            context_gap_kind: Some(ContextGapKind::MissingEvidence),
            minimum_context_gap_priority: Some(Confidence::new(0.7)?),
            ..CellLookup::default()
        })?;

        assert_eq!(results, vec![high_priority]);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_filters_by_invalidation_condition_kind_and_priority(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-invalidation-condition");
        let routine = sample_cell("project:continuitydb:invalidation-routine", 0.91, 12)?;
        let mut low_priority =
            sample_cell("project:continuitydb:invalidation-low-priority", 0.83, 15)?;
        low_priority.add_invalidation_condition(InvalidationCondition::new(
            InvalidationConditionKind::DependencyInvalidated,
            "a low-impact dependency is superseded",
            "low-priority falsifier should not cross the threshold",
            0.4,
        )?);
        let mut high_priority =
            sample_cell("project:continuitydb:invalidation-high-priority", 0.83, 15)?;
        high_priority.add_invalidation_condition(InvalidationCondition::new(
            InvalidationConditionKind::DependencyInvalidated,
            "a retained dependency artifact is superseded",
            "high-priority falsifier should remain retrievable",
            0.9,
        )?);
        {
            let mut kernel = FileKernel::open(&path)?;
            append_committed(&mut kernel, routine)?;
            append_committed(&mut kernel, low_priority)?;
            high_priority = append_committed(&mut kernel, high_priority)?;
        }

        let reopened = FileKernel::open(&path)?;
        let results = reopened.lookup_cells(CellLookup {
            invalidation_condition_kind: Some(InvalidationConditionKind::DependencyInvalidated),
            minimum_invalidation_priority: Some(Confidence::new(0.7)?),
            ..CellLookup::default()
        })?;

        assert_eq!(results, vec![high_priority]);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_invalidation_condition_lookup_uses_persistent_index_addresses(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-invalidation-index-lookup");
        let index_path = temp_persistent_index_path(&path);
        let expected = {
            let mut kernel = FileKernel::open(&path)?;
            append_committed(
                &mut kernel,
                sample_cell(
                    "project:continuitydb:persistent-invalidation-unrelated",
                    0.91,
                    12,
                )?,
            )?;
            let mut low_priority =
                sample_cell("project:continuitydb:persistent-invalidation-low", 0.83, 15)?;
            low_priority.add_invalidation_condition(InvalidationCondition::new(
                InvalidationConditionKind::DependencyInvalidated,
                "a low-impact dependency may be stale",
                "low priority invalidation condition should stay below threshold",
                0.4,
            )?);
            let mut high_priority = sample_cell(
                "project:continuitydb:persistent-invalidation-high",
                0.83,
                15,
            )?;
            high_priority.add_invalidation_condition(InvalidationCondition::new(
                InvalidationConditionKind::DependencyInvalidated,
                "a retained dependency artifact was superseded",
                "high priority invalidation condition should be durable-index retrievable",
                0.9,
            )?);
            append_committed(&mut kernel, low_priority)?;
            append_committed(&mut kernel, high_priority)?
        };
        let checkpoint: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&index_path)?)?;
        assert_eq!(
            checkpoint["invalidation_condition_kind_address_count"],
            serde_json::json!(2)
        );
        assert_eq!(
            checkpoint["invalidation_condition_priority_address_count"],
            serde_json::json!(2)
        );
        let mut reopened = FileKernel::open(&path)?;
        reopened.index.cells.clear();

        let cells = reopened.lookup_cells(CellLookup {
            invalidation_condition_kind: Some(InvalidationConditionKind::DependencyInvalidated),
            minimum_invalidation_priority: Some(Confidence::new(0.7)?),
            ..CellLookup::default()
        })?;

        assert_eq!(vec![expected], cells);

        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_lookup_plan_indexes_invalidation_condition_constraints(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-invalidation-lookup-plan");
        let mut kernel = FileKernel::open(&path)?;
        append_committed(
            &mut kernel,
            sample_cell("project:continuitydb:invalidation-plan-routine", 0.91, 12)?,
        )?;
        let mut invalidation =
            sample_cell("project:continuitydb:invalidation-plan-high", 0.83, 15)?;
        invalidation.add_invalidation_condition(InvalidationCondition::new(
            InvalidationConditionKind::DependencyInvalidated,
            "the predecessor state has been invalidated",
            "dependency invalidation should be directly materializable",
            0.85,
        )?);
        append_committed(&mut kernel, invalidation)?;

        let plan = kernel.lookup_plan(&CellLookup {
            invalidation_condition_kind: Some(InvalidationConditionKind::DependencyInvalidated),
            minimum_invalidation_priority: Some(Confidence::new(0.7)?),
            ..CellLookup::default()
        });

        assert_eq!(
            plan.indexed_constraints,
            vec![
                "invalidation_condition_kind",
                "minimum_invalidation_priority"
            ]
        );
        assert_eq!(plan.residual_exact_constraints, Vec::<&'static str>::new());
        assert_eq!(plan.candidate_count, 1);
        assert!(!plan.full_scan);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_context_gap_lookup_uses_persistent_index_addresses(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-context-gap-index-lookup");
        let index_path = temp_persistent_index_path(&path);
        let expected = {
            let mut kernel = FileKernel::open(&path)?;
            append_committed(
                &mut kernel,
                sample_cell(
                    "project:continuitydb:persistent-context-gap-unrelated",
                    0.91,
                    12,
                )?,
            )?;
            let mut low_priority =
                sample_cell("project:continuitydb:persistent-context-gap-low", 0.83, 15)?;
            low_priority.add_context_gap(ContextGap::new(
                ContextGapKind::MissingEvidence,
                "which artifact supports the secondary claim?",
                "low priority gap should stay below the checkout threshold",
                0.4,
            )?);
            let mut high_priority =
                sample_cell("project:continuitydb:persistent-context-gap-high", 0.83, 15)?;
            high_priority.add_context_gap(ContextGap::new(
                ContextGapKind::MissingEvidence,
                "which retained artifact proves the live benchmark?",
                "high priority missing evidence should be durable-index retrievable",
                0.9,
            )?);
            append_committed(&mut kernel, low_priority)?;
            append_committed(&mut kernel, high_priority)?
        };
        let checkpoint: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&index_path)?)?;
        assert_eq!(
            checkpoint["context_gap_kind_address_count"],
            serde_json::json!(2)
        );
        assert_eq!(
            checkpoint["context_gap_priority_address_count"],
            serde_json::json!(2)
        );
        let mut reopened = FileKernel::open(&path)?;
        reopened.index.cells.clear();

        let cells = reopened.lookup_cells(CellLookup {
            context_gap_kind: Some(ContextGapKind::MissingEvidence),
            minimum_context_gap_priority: Some(Confidence::new(0.7)?),
            ..CellLookup::default()
        })?;

        assert_eq!(vec![expected], cells);

        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_lookup_plan_indexes_context_gap_constraints(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-context-gap-lookup-plan");
        let mut kernel = FileKernel::open(&path)?;
        append_committed(
            &mut kernel,
            sample_cell("project:continuitydb:context-gap-plan-routine", 0.91, 12)?,
        )?;
        let mut gap = sample_cell("project:continuitydb:context-gap-plan-high", 0.83, 15)?;
        gap.add_context_gap(ContextGap::new(
            ContextGapKind::MissingDependency,
            "which predecessor decision constrains this claim?",
            "missing dependency should be directly materializable",
            0.85,
        )?);
        append_committed(&mut kernel, gap)?;

        let plan = kernel.lookup_plan(&CellLookup {
            context_gap_kind: Some(ContextGapKind::MissingDependency),
            minimum_context_gap_priority: Some(Confidence::new(0.7)?),
            ..CellLookup::default()
        });

        assert_eq!(
            plan.indexed_constraints,
            vec!["context_gap_kind", "minimum_context_gap_priority"]
        );
        assert_eq!(plan.residual_exact_constraints, Vec::<&'static str>::new());
        assert_eq!(plan.candidate_count, 1);
        assert!(!plan.full_scan);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_lookup_plan_reports_selection_reason_as_residual_exact(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-selection-reason-plan");
        let mut salient = sample_cell(
            "project:continuitydb:selection-reason-plan-attention",
            0.83,
            15,
        )?;
        salient.set_attention(AttentionSignal::new(0.8, 0.7, 0.9, 0.6)?);
        let mut kernel = FileKernel::open(&path)?;
        append_committed(&mut kernel, salient)?;

        let plan = kernel.lookup_plan(&CellLookup {
            selection_reason: Some(ContextPacketSelectionReason::AttentionSignal),
            ..CellLookup::default()
        });

        assert_eq!(plan.exact_constraints, vec!["selection_reason"]);
        assert_eq!(plan.residual_exact_constraints, vec!["selection_reason"]);
        assert!(plan.full_scan);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rebuilds_answerability_question_index() -> Result<(), Box<dyn std::error::Error>>
    {
        let path = temp_kernel_path("continuitydb-file-kernel-answerability-index-rebuild");
        let mut status = sample_cell("project:continuitydb:index-status", 0.91, 12)?;
        status.answerability = Answerability::new(vec!["what is status?".to_string()])?;
        let mut frontier = sample_cell("project:continuitydb:index-frontier", 0.83, 15)?;
        frontier.answerability = Answerability::new(vec!["what is frontier?".to_string()])?;
        {
            let mut kernel = FileKernel::open(&path)?;
            append_committed(&mut kernel, status)?;
            frontier = append_committed(&mut kernel, frontier)?;
        }

        let reopened = FileKernel::open(&path)?;
        let positions = reopened
            .index
            .answerability_questions
            .get("what is frontier?")
            .cloned()
            .unwrap_or_default();
        let indexed = positions
            .iter()
            .map(|position| reopened.index.cells[*position].clone())
            .collect::<Vec<_>>();

        assert_eq!(indexed, vec![frontier]);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_updates_answerability_question_index_after_append(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-answerability-index-append");
        let mut frontier = sample_cell("project:continuitydb:index-append-frontier", 0.83, 15)?;
        frontier.answerability = Answerability::new(vec!["what changed?".to_string()])?;
        let mut kernel = FileKernel::open(&path)?;

        frontier = append_committed(&mut kernel, frontier)?;
        let positions = kernel
            .index
            .answerability_questions
            .get("what changed?")
            .cloned()
            .unwrap_or_default();
        let indexed = positions
            .iter()
            .map(|position| kernel.index.cells[*position].clone())
            .collect::<Vec<_>>();

        assert_eq!(indexed, vec![frontier]);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_filters_by_evidence_source() -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-evidence-source");
        let observed =
            sample_cell_with_source("project:continuitydb:observed", "sensor", 0.91, 12)?;
        let mut reviewed =
            sample_cell_with_source("project:continuitydb:reviewed", "human", 0.83, 15)?;
        {
            let mut kernel = FileKernel::open(&path)?;
            append_committed(&mut kernel, observed)?;
            reviewed = append_committed(&mut kernel, reviewed)?;
        }

        let reopened = FileKernel::open(&path)?;
        let results = reopened.lookup_cells(CellLookup {
            evidence_source: Some("human".to_string()),
            ..CellLookup::default()
        })?;

        assert_eq!(results, vec![reviewed]);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rebuilds_evidence_source_index() -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-evidence-index-rebuild");
        let observed =
            sample_cell_with_source("project:continuitydb:index-observed", "sensor", 0.91, 12)?;
        let mut reviewed =
            sample_cell_with_source("project:continuitydb:index-reviewed", "human", 0.83, 15)?;
        {
            let mut kernel = FileKernel::open(&path)?;
            append_committed(&mut kernel, observed)?;
            reviewed = append_committed(&mut kernel, reviewed)?;
        }

        let reopened = FileKernel::open(&path)?;
        let positions = reopened
            .index
            .evidence_sources
            .get("human")
            .cloned()
            .unwrap_or_default();
        let indexed = positions
            .iter()
            .map(|position| reopened.index.cells[*position].clone())
            .collect::<Vec<_>>();

        assert_eq!(indexed, vec![reviewed]);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_updates_evidence_source_index_after_append(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-evidence-index-append");
        let mut reviewed = sample_cell_with_source(
            "project:continuitydb:index-append-reviewed",
            "human",
            0.83,
            15,
        )?;
        let mut kernel = FileKernel::open(&path)?;

        reviewed = append_committed(&mut kernel, reviewed)?;
        let positions = kernel
            .index
            .evidence_sources
            .get("human")
            .cloned()
            .unwrap_or_default();
        let indexed = positions
            .iter()
            .map(|position| kernel.index.cells[*position].clone())
            .collect::<Vec<_>>();

        assert_eq!(indexed, vec![reviewed]);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rebuilds_scope_index() -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-scope-index-reopen");
        let mut project = sample_cell("project:continuitydb:scope-index-project", 0.91, 12)?;
        let mut team = sample_cell("project:continuitydb:scope-index-team", 0.83, 15)?;
        project.scope = Scope::Project("continuitydb".to_string());
        team.scope = Scope::Team("storage".to_string());
        let expected_project;
        {
            let mut kernel = FileKernel::open(&path)?;
            expected_project = append_committed(&mut kernel, project)?;
            append_committed(&mut kernel, team)?;
        }

        let reopened = FileKernel::open(&path)?;
        let positions = reopened
            .index
            .scopes
            .get(&Scope::Project("continuitydb".to_string()))
            .cloned()
            .unwrap_or_default();
        let indexed = positions
            .iter()
            .map(|position| reopened.index.cells[*position].clone())
            .collect::<Vec<_>>();
        let lookup_results = reopened.lookup_cells(CellLookup {
            scope: Some(Scope::Project("continuitydb".to_string())),
            ..CellLookup::default()
        })?;

        assert_eq!(indexed, vec![expected_project.clone()]);
        assert_eq!(lookup_results, vec![expected_project]);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_updates_scope_index_after_append() -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-scope-index-append");
        let mut task = sample_cell("project:continuitydb:scope-index-task", 0.83, 15)?;
        task.scope = Scope::Task("production-storage".to_string());
        let mut kernel = FileKernel::open(&path)?;

        let expected = append_committed(&mut kernel, task)?;
        let positions = kernel
            .index
            .scopes
            .get(&Scope::Task("production-storage".to_string()))
            .cloned()
            .unwrap_or_default();
        let indexed = positions
            .iter()
            .map(|position| kernel.index.cells[*position].clone())
            .collect::<Vec<_>>();

        assert_eq!(indexed, vec![expected]);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_filters_by_minimum_confidence() -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-minimum-confidence");
        let weak = sample_cell("project:continuitydb:weak", 0.61, 12)?;
        let mut strong = sample_cell("project:continuitydb:strong", 0.86, 15)?;
        {
            let mut kernel = FileKernel::open(&path)?;
            append_committed(&mut kernel, weak)?;
            strong = append_committed(&mut kernel, strong)?;
        }

        let reopened = FileKernel::open(&path)?;
        let results = reopened.lookup_cells(CellLookup {
            minimum_confidence: Some(Confidence::new(0.8)?),
            ..CellLookup::default()
        })?;

        assert_eq!(results, vec![strong]);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rebuilds_confidence_index() -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-confidence-index-reopen");
        let weak = sample_cell("project:continuitydb:confidence-index-weak", 0.61, 12)?;
        let mut strong = sample_cell("project:continuitydb:confidence-index-strong", 0.86, 15)?;
        {
            let mut kernel = FileKernel::open(&path)?;
            append_committed(&mut kernel, weak)?;
            strong = append_committed(&mut kernel, strong)?;
        }

        let reopened = FileKernel::open(&path)?;
        let indexed = reopened
            .index
            .max_evidence_confidences
            .iter()
            .filter(|(confidence, _position)| *confidence >= 0.8)
            .map(|(_confidence, position)| reopened.index.cells[*position].clone())
            .collect::<Vec<_>>();
        let lookup_results = reopened.lookup_cells(CellLookup {
            minimum_confidence: Some(Confidence::new(0.8)?),
            ..CellLookup::default()
        })?;

        assert_eq!(indexed, vec![strong.clone()]);
        assert_eq!(lookup_results, vec![strong]);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_updates_confidence_index_after_append() -> Result<(), Box<dyn std::error::Error>>
    {
        let path = temp_kernel_path("continuitydb-file-kernel-confidence-index-append");
        let strong = sample_cell("project:continuitydb:confidence-index-append", 0.91, 12)?;
        let mut kernel = FileKernel::open(&path)?;

        let expected = append_committed(&mut kernel, strong)?;
        let indexed = kernel
            .index
            .max_evidence_confidences
            .iter()
            .filter(|(confidence, _position)| *confidence >= 0.9)
            .map(|(_confidence, position)| kernel.index.cells[*position].clone())
            .collect::<Vec<_>>();

        assert_eq!(indexed, vec![expected]);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_filters_by_system_time() -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-system-time");
        let committed_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let before_commit = Utc
            .with_ymd_and_hms(2026, 5, 20, 11, 59, 59)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let cell = sample_cell("project:continuitydb:system-time", 0.91, 12)?;
        {
            let mut kernel = FileKernel::open(&path)?;
            kernel.append_cell_at(cell.clone(), committed_at)?;
        }

        let reopened = FileKernel::open(&path)?;
        let current = reopened.lookup_cells(CellLookup {
            system_at: Some(committed_at),
            ..CellLookup::default()
        })?;
        let historical = reopened.lookup_cells(CellLookup {
            system_at: Some(before_commit),
            ..CellLookup::default()
        })?;

        assert_eq!(current.len(), 1);
        assert_eq!(current[0].id, cell.id);
        assert_eq!(current[0].system_time.from(), committed_at);
        assert!(historical.is_empty());
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rebuilds_system_time_index() -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-system-time-index-reopen");
        let first_commit = Utc
            .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let second_commit = Utc
            .with_ymd_and_hms(2026, 5, 20, 13, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let first = sample_cell("project:continuitydb:system-time-index-first", 0.91, 12)?;
        let second = sample_cell("project:continuitydb:system-time-index-second", 0.83, 15)?;
        {
            let mut kernel = FileKernel::open(&path)?;
            kernel.append_cell_at(first.clone(), first_commit)?;
            kernel.append_cell_at(second, second_commit)?;
        }

        let reopened = FileKernel::open(&path)?;
        let indexed = reopened
            .index
            .system_times
            .iter()
            .filter(|(system_from, _position)| *system_from <= first_commit)
            .map(|(_system_from, position)| reopened.index.cells[*position].clone())
            .collect::<Vec<_>>();
        let lookup_results = reopened.lookup_cells(CellLookup {
            system_at: Some(first_commit),
            ..CellLookup::default()
        })?;

        assert_eq!(indexed.len(), 1);
        assert_eq!(indexed[0].id, first.id);
        assert_eq!(lookup_results.len(), 1);
        assert_eq!(lookup_results[0].id, first.id);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_updates_system_time_index_after_append() -> Result<(), Box<dyn std::error::Error>>
    {
        let path = temp_kernel_path("continuitydb-file-kernel-system-time-index-append");
        let committed_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let cell = sample_cell("project:continuitydb:system-time-index-append", 0.91, 12)?;
        let mut kernel = FileKernel::open(&path)?;

        kernel.append_cell_at(cell.clone(), committed_at)?;
        let indexed = kernel
            .index
            .system_times
            .iter()
            .filter(|(system_from, _position)| *system_from <= committed_at)
            .map(|(_system_from, position)| kernel.index.cells[*position].clone())
            .collect::<Vec<_>>();

        assert_eq!(indexed.len(), 1);
        assert_eq!(indexed[0].id, cell.id);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rebuilds_valid_time_index() -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-valid-time-index-reopen");
        let first_valid = Utc
            .with_ymd_and_hms(2026, 5, 20, 0, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let second_valid = Utc
            .with_ymd_and_hms(2026, 5, 21, 0, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let mut first = sample_cell("project:continuitydb:valid-time-index-first", 0.91, 12)?;
        let mut second = sample_cell("project:continuitydb:valid-time-index-second", 0.83, 15)?;
        first.valid_time = ValidTimeRange::new(first_valid, None)?;
        second.valid_time = ValidTimeRange::new(second_valid, None)?;
        let expected_first;
        {
            let mut kernel = FileKernel::open(&path)?;
            expected_first = append_committed(&mut kernel, first)?;
            append_committed(&mut kernel, second)?;
        }

        let reopened = FileKernel::open(&path)?;
        let indexed = reopened
            .index
            .valid_times
            .iter()
            .filter(|(valid_from, _position)| *valid_from <= first_valid)
            .map(|(_valid_from, position)| reopened.index.cells[*position].clone())
            .collect::<Vec<_>>();
        let lookup_results = reopened.lookup_cells(CellLookup {
            valid_at: Some(first_valid),
            ..CellLookup::default()
        })?;

        assert_eq!(indexed, vec![expected_first.clone()]);
        assert_eq!(lookup_results, vec![expected_first]);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_updates_valid_time_index_after_append() -> Result<(), Box<dyn std::error::Error>>
    {
        let path = temp_kernel_path("continuitydb-file-kernel-valid-time-index-append");
        let valid_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 0, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let mut cell = sample_cell("project:continuitydb:valid-time-index-append", 0.91, 12)?;
        cell.valid_time = ValidTimeRange::new(valid_at, None)?;
        let mut kernel = FileKernel::open(&path)?;

        let expected = append_committed(&mut kernel, cell)?;
        let indexed = kernel
            .index
            .valid_times
            .iter()
            .filter(|(valid_from, _position)| *valid_from <= valid_at)
            .map(|(_valid_from, position)| kernel.index.cells[*position].clone())
            .collect::<Vec<_>>();

        assert_eq!(indexed, vec![expected]);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_candidate_selection_prefers_smallest_indexed_constraint(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-smallest-candidate-index");
        let mut narrow = sample_cell("project:continuitydb:candidate-index-narrow", 0.91, 12)?;
        narrow.answerability = Answerability::new(vec!["what changed?".to_string()])?;
        let narrow_id = narrow.id;
        let broad_first =
            sample_cell("project:continuitydb:candidate-index-broad-first", 0.83, 15)?;
        let broad_second = sample_cell(
            "project:continuitydb:candidate-index-broad-second",
            0.82,
            15,
        )?;
        let mut kernel = FileKernel::open(&path)?;
        append_committed(&mut kernel, broad_first)?;
        append_committed(&mut kernel, narrow)?;
        append_committed(&mut kernel, broad_second)?;
        let narrow_position = kernel
            .index
            .position_by_id(narrow_id)
            .ok_or_else(|| std::io::Error::other("missing indexed narrow cell"))?;
        let lookup = CellLookup {
            scope: Some(Scope::Project("continuitydb".to_string())),
            answerability_question: Some("what changed?".to_string()),
            ..CellLookup::default()
        };

        let candidate_positions = kernel.index.candidate_positions(&lookup);
        let lookup_results = kernel.lookup_cells(lookup)?;

        assert_eq!(candidate_positions, vec![narrow_position]);
        assert_eq!(lookup_results.len(), 1);
        assert_eq!(lookup_results[0].id, narrow_id);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_candidate_selection_intersects_indexed_constraints(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-candidate-index-intersection");
        let mut matching = sample_cell(
            "project:continuitydb:candidate-index-intersection-match",
            0.91,
            12,
        )?;
        matching.answerability = Answerability::new(vec!["what changed?".to_string()])?;
        let matching_id = matching.id;
        let broad_first = sample_cell(
            "project:continuitydb:candidate-index-intersection-broad-first",
            0.83,
            15,
        )?;
        let broad_second = sample_cell(
            "project:continuitydb:candidate-index-intersection-broad-second",
            0.82,
            15,
        )?;
        let mut wrong_scope = sample_cell(
            "project:continuitydb:candidate-index-intersection-wrong-scope",
            0.89,
            11,
        )?;
        wrong_scope.scope = Scope::Team("platform".to_string());
        wrong_scope.answerability = Answerability::new(vec!["what changed?".to_string()])?;
        let mut kernel = FileKernel::open(&path)?;
        append_committed(&mut kernel, broad_first)?;
        append_committed(&mut kernel, matching)?;
        append_committed(&mut kernel, broad_second)?;
        append_committed(&mut kernel, wrong_scope)?;
        let matching_position = kernel
            .index
            .position_by_id(matching_id)
            .ok_or_else(|| std::io::Error::other("missing indexed matching cell"))?;
        let lookup = CellLookup {
            scope: Some(Scope::Project("continuitydb".to_string())),
            answerability_question: Some("what changed?".to_string()),
            ..CellLookup::default()
        };

        let candidate_positions = kernel.index.candidate_positions(&lookup);
        let lookup_results = kernel.lookup_cells(lookup)?;

        assert_eq!(candidate_positions, vec![matching_position]);
        assert_eq!(lookup_results.len(), 1);
        assert_eq!(lookup_results[0].id, matching_id);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_lookup_plan_reports_full_scan_without_indexed_constraints(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-lookup-plan-full-scan");
        let first = sample_cell("project:continuitydb:lookup-plan-full-scan-first", 0.91, 12)?;
        let second = sample_cell(
            "project:continuitydb:lookup-plan-full-scan-second",
            0.83,
            15,
        )?;
        let mut kernel = FileKernel::open(&path)?;
        append_committed(&mut kernel, first)?;
        append_committed(&mut kernel, second)?;

        let plan = kernel.lookup_plan(&CellLookup::default());

        assert_eq!(plan.indexed_constraint_count, 0);
        assert!(plan.indexed_constraints.is_empty());
        assert_eq!(plan.candidate_count, 2);
        assert!(plan.full_scan);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_lookup_plan_reports_intersected_indexed_candidates(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-lookup-plan-indexed");
        let mut matching = sample_cell("project:continuitydb:lookup-plan-indexed-match", 0.91, 12)?;
        matching.answerability = Answerability::new(vec!["what changed?".to_string()])?;
        let broad = sample_cell("project:continuitydb:lookup-plan-indexed-broad", 0.83, 15)?;
        let mut wrong_scope = sample_cell(
            "project:continuitydb:lookup-plan-indexed-wrong-scope",
            0.89,
            11,
        )?;
        wrong_scope.scope = Scope::Team("platform".to_string());
        wrong_scope.answerability = Answerability::new(vec!["what changed?".to_string()])?;
        let mut kernel = FileKernel::open(&path)?;
        append_committed(&mut kernel, broad)?;
        append_committed(&mut kernel, matching)?;
        append_committed(&mut kernel, wrong_scope)?;
        let lookup = CellLookup {
            scope: Some(Scope::Project("continuitydb".to_string())),
            answerability_question: Some("what changed?".to_string()),
            ..CellLookup::default()
        };

        let plan = kernel.lookup_plan(&lookup);

        assert_eq!(plan.indexed_constraint_count, 2);
        assert_eq!(
            plan.indexed_constraints,
            vec!["scope", "answerability_question"]
        );
        assert_eq!(
            plan.indexed_constraint_plans,
            vec![
                FileKernelIndexedConstraintPlan {
                    name: "scope",
                    candidate_count: 2,
                },
                FileKernelIndexedConstraintPlan {
                    name: "answerability_question",
                    candidate_count: 2,
                },
            ]
        );
        assert_eq!(plan.candidate_count, 1);
        assert!(!plan.full_scan);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_lookup_plan_uses_persistent_index_records(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-lookup-plan-persistent-index");
        let index_path = temp_persistent_index_path(&path);
        let mut matching = sample_cell(
            "project:continuitydb:lookup-plan-persistent-index-match",
            0.91,
            12,
        )?;
        matching.answerability = Answerability::new(vec!["what changed?".to_string()])?;
        let broad = sample_cell(
            "project:continuitydb:lookup-plan-persistent-index-broad",
            0.83,
            15,
        )?;
        let mut wrong_scope = sample_cell(
            "project:continuitydb:lookup-plan-persistent-index-wrong-scope",
            0.89,
            11,
        )?;
        wrong_scope.scope = Scope::Team("platform".to_string());
        wrong_scope.answerability = Answerability::new(vec!["what changed?".to_string()])?;
        {
            let mut kernel = FileKernel::open(&path)?;
            append_committed(&mut kernel, broad)?;
            append_committed(&mut kernel, matching)?;
            append_committed(&mut kernel, wrong_scope)?;
        }
        let mut reopened = FileKernel::open(&path)?;
        reopened.index.cells.clear();
        reopened.index.scopes.clear();
        reopened.index.answerability_questions.clear();
        let lookup = CellLookup {
            scope: Some(Scope::Project("continuitydb".to_string())),
            answerability_question: Some("what changed?".to_string()),
            ..CellLookup::default()
        };

        let plan = reopened.lookup_plan(&lookup);

        assert_eq!(plan.indexed_constraint_count, 2);
        assert_eq!(
            plan.indexed_constraints,
            vec!["scope", "answerability_question"]
        );
        assert_eq!(
            plan.indexed_constraint_plans,
            vec![
                FileKernelIndexedConstraintPlan {
                    name: "scope",
                    candidate_count: 2,
                },
                FileKernelIndexedConstraintPlan {
                    name: "answerability_question",
                    candidate_count: 2,
                },
            ]
        );
        assert_eq!(plan.candidate_count, 1);
        assert!(!plan.full_scan);

        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_lookup_plan_reports_exact_match_count_after_filtering(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-lookup-plan-exact-count");
        let first_valid = Utc
            .with_ymd_and_hms(2026, 5, 20, 0, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let first_expired = Utc
            .with_ymd_and_hms(2026, 5, 21, 0, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let as_of = Utc
            .with_ymd_and_hms(2026, 5, 22, 0, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let mut expired = sample_cell("project:continuitydb:lookup-plan-expired", 0.91, 12)?;
        expired.valid_time = ValidTimeRange::new(first_valid, Some(first_expired))?;
        let mut current = sample_cell("project:continuitydb:lookup-plan-current", 0.83, 15)?;
        current.valid_time = ValidTimeRange::new(first_valid, None)?;
        let mut kernel = FileKernel::open(&path)?;
        append_committed(&mut kernel, expired)?;
        append_committed(&mut kernel, current)?;

        let plan = kernel.lookup_plan(&CellLookup {
            valid_at: Some(as_of),
            ..CellLookup::default()
        });

        assert_eq!(plan.indexed_constraints, vec!["valid_at"]);
        assert_eq!(plan.candidate_count, 2);
        assert_eq!(plan.exact_match_count, 1);
        assert!(!plan.full_scan);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_lookup_plan_reports_filtered_candidate_count(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-lookup-plan-filtered-count");
        let first_valid = Utc
            .with_ymd_and_hms(2026, 5, 20, 0, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let first_expired = Utc
            .with_ymd_and_hms(2026, 5, 21, 0, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let as_of = Utc
            .with_ymd_and_hms(2026, 5, 22, 0, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let mut expired = sample_cell(
            "project:continuitydb:lookup-plan-filtered-expired",
            0.91,
            12,
        )?;
        expired.valid_time = ValidTimeRange::new(first_valid, Some(first_expired))?;
        let mut current = sample_cell(
            "project:continuitydb:lookup-plan-filtered-current",
            0.83,
            15,
        )?;
        current.valid_time = ValidTimeRange::new(first_valid, None)?;
        let mut kernel = FileKernel::open(&path)?;
        append_committed(&mut kernel, expired)?;
        append_committed(&mut kernel, current)?;

        let plan = kernel.lookup_plan(&CellLookup {
            valid_at: Some(as_of),
            ..CellLookup::default()
        });

        assert_eq!(plan.candidate_count, 2);
        assert_eq!(plan.exact_match_count, 1);
        assert_eq!(plan.filtered_candidate_count, 1);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_lookup_plan_reports_candidate_selectivity(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-lookup-plan-selectivity");
        let first_valid = Utc
            .with_ymd_and_hms(2026, 5, 20, 0, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let first_expired = Utc
            .with_ymd_and_hms(2026, 5, 21, 0, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let as_of = Utc
            .with_ymd_and_hms(2026, 5, 22, 0, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let mut expired = sample_cell(
            "project:continuitydb:lookup-plan-selectivity-expired",
            0.91,
            12,
        )?;
        expired.valid_time = ValidTimeRange::new(first_valid, Some(first_expired))?;
        let mut current = sample_cell(
            "project:continuitydb:lookup-plan-selectivity-current",
            0.83,
            15,
        )?;
        current.valid_time = ValidTimeRange::new(first_valid, None)?;
        let mut kernel = FileKernel::open(&path)?;
        append_committed(&mut kernel, expired)?;
        append_committed(&mut kernel, current)?;

        let plan = kernel.lookup_plan(&CellLookup {
            valid_at: Some(as_of),
            ..CellLookup::default()
        });

        assert_eq!(plan.candidate_count, 2);
        assert_eq!(plan.exact_match_count, 1);
        assert_eq!(plan.candidate_selectivity_basis_points, 5000);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_lookup_plan_reports_exact_constraint_labels(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-lookup-plan-exact-constraints");
        let valid_from = Utc
            .with_ymd_and_hms(2026, 5, 20, 0, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let as_of = Utc
            .with_ymd_and_hms(2026, 5, 22, 0, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let mut matching = sample_cell(
            "project:continuitydb:lookup-plan-exact-constraints",
            0.91,
            12,
        )?;
        matching.valid_time = ValidTimeRange::new(valid_from, None)?;
        let mut kernel = FileKernel::open(&path)?;
        append_committed(&mut kernel, matching)?;

        let plan = kernel.lookup_plan(&CellLookup {
            scope: Some(Scope::Project("continuitydb".to_string())),
            valid_at: Some(as_of),
            ..CellLookup::default()
        });

        assert_eq!(plan.exact_constraint_count, 2);
        assert_eq!(plan.exact_constraints, vec!["scope", "valid_at"]);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_lookup_plan_reports_residual_exact_constraints(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path =
            temp_kernel_path("continuitydb-file-kernel-lookup-plan-residual-exact-constraints");
        let valid_from = Utc
            .with_ymd_and_hms(2026, 5, 20, 0, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let as_of = Utc
            .with_ymd_and_hms(2026, 5, 22, 0, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let mut matching = sample_cell(
            "project:continuitydb:lookup-plan-residual-exact-constraints",
            0.91,
            12,
        )?;
        matching.valid_time = ValidTimeRange::new(valid_from, None)?;
        let mut kernel = FileKernel::open(&path)?;
        append_committed(&mut kernel, matching)?;

        let plan = kernel.lookup_plan(&CellLookup {
            scope: Some(Scope::Project("continuitydb".to_string())),
            valid_at: Some(as_of),
            ..CellLookup::default()
        });

        assert_eq!(plan.residual_exact_constraint_count, 1);
        assert_eq!(plan.residual_exact_constraints, vec!["valid_at"]);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_lookup_plan_reports_epistemic_action_reason_as_indexed_exact(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-action-reason-plan");
        let mut cell = sample_cell("project:continuitydb:action-reason-plan", 0.91, 12)?;
        let expectation = EpistemicExpectation::from_expected_outcome(
            "release asset exists",
            Confidence::new(0.9)?,
            false,
        )?;
        cell.set_uncertainty(EpistemicUncertainty::from_expectation(
            Confidence::new(0.22)?,
            expectation,
            "baseline failed but confidence remains low",
        )?);
        let mut kernel = FileKernel::open(&path)?;
        append_committed(&mut kernel, cell)?;

        let plan = kernel.lookup_plan(&CellLookup {
            epistemic_action_reason: Some(EpistemicActionReason::HighSurprise),
            ..CellLookup::default()
        });

        assert_eq!(plan.indexed_constraint_count, 1);
        assert_eq!(plan.indexed_constraints, vec!["epistemic_action_reason"]);
        assert_eq!(plan.residual_exact_constraints, Vec::<&'static str>::new());
        assert_eq!(plan.candidate_count, 1);
        assert!(!plan.full_scan);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_lookup_plan_indexes_epistemic_action_reason(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-action-reason-index-plan");
        let mut surprising = sample_cell(
            "project:continuitydb:action-reason-index-surprise",
            0.91,
            12,
        )?;
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
        let mut uncertain = sample_cell(
            "project:continuitydb:action-reason-index-uncertain",
            0.91,
            12,
        )?;
        uncertain.set_uncertainty(EpistemicUncertainty::new(
            Confidence::new(0.82)?,
            0.5,
            "uncertain but not surprising",
        )?);
        let mut kernel = FileKernel::open(&path)?;
        append_committed(&mut kernel, surprising)?;
        append_committed(&mut kernel, uncertain)?;

        let plan = kernel.lookup_plan(&CellLookup {
            epistemic_action_reason: Some(EpistemicActionReason::HighSurprise),
            ..CellLookup::default()
        });

        assert_eq!(plan.indexed_constraints, vec!["epistemic_action_reason"]);
        assert_eq!(plan.residual_exact_constraints, Vec::<&'static str>::new());
        assert_eq!(plan.candidate_count, 1);
        assert!(!plan.full_scan);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_epistemic_action_reason_lookup_uses_persistent_index_addresses(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-action-reason-persistent-lookup");
        let index_path = temp_persistent_index_path(&path);
        let expected = {
            let mut kernel = FileKernel::open(&path)?;
            let mut surprising = sample_cell(
                "project:continuitydb:action-reason-persistent-surprise",
                0.91,
                12,
            )?;
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
            let mut uncertain = sample_cell(
                "project:continuitydb:action-reason-persistent-uncertain",
                0.91,
                12,
            )?;
            uncertain.set_uncertainty(EpistemicUncertainty::new(
                Confidence::new(0.82)?,
                0.5,
                "uncertain but not surprising",
            )?);
            let expected = append_committed(&mut kernel, surprising)?;
            append_committed(&mut kernel, uncertain)?;
            expected
        };
        let mut checkpoint: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&index_path)?)?;
        checkpoint["cell_record_count"] = serde_json::json!(0);
        checkpoint["cells"] = serde_json::json!([]);
        fs::write(&index_path, serde_json::to_vec(&checkpoint)?)?;
        let mut reopened = FileKernel::open(&path)?;
        reopened.index.epistemic_action_reasons.clear();
        reopened.index.cells.clear();

        let cells = reopened.lookup_cells(CellLookup {
            epistemic_action_reason: Some(EpistemicActionReason::HighSurprise),
            ..CellLookup::default()
        })?;

        assert_eq!(cells, vec![expected]);
        fs::remove_file(path)?;
        fs::remove_file(index_path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_lookup_plan_reports_lossy_indexed_constraints(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-lookup-plan-lossy-constraints");
        let valid_from = Utc
            .with_ymd_and_hms(2026, 5, 20, 0, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let as_of = Utc
            .with_ymd_and_hms(2026, 5, 22, 0, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let mut matching = sample_cell(
            "project:continuitydb:lookup-plan-lossy-constraints",
            0.91,
            12,
        )?;
        matching.valid_time = ValidTimeRange::new(valid_from, None)?;
        let mut kernel = FileKernel::open(&path)?;
        append_committed(&mut kernel, matching)?;

        let plan = kernel.lookup_plan(&CellLookup {
            scope: Some(Scope::Project("continuitydb".to_string())),
            system_at: Some(as_of),
            valid_at: Some(as_of),
            ..CellLookup::default()
        });

        assert_eq!(plan.lossy_indexed_constraint_count, 2);
        assert_eq!(
            plan.lossy_indexed_constraints,
            vec!["system_at", "valid_at"]
        );
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_lookup_plan_reports_dependency_kind_index(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-lookup-plan-dependency-kind");
        let target = StateCellId::new();
        let other_target = StateCellId::new();
        let mut first = sample_cell("project:continuitydb:lookup-plan-kind-first", 0.9, 12)?;
        first.dependencies.push(CellDependency::new(
            target,
            CellDependencyKind::DependsOn,
            "depends on target",
        ));
        let mut support = sample_cell("project:continuitydb:lookup-plan-kind-support", 0.9, 12)?;
        support.dependencies.push(CellDependency::new(
            target,
            CellDependencyKind::Supports,
            "supports target",
        ));
        let mut second = sample_cell("project:continuitydb:lookup-plan-kind-second", 0.9, 12)?;
        second.dependencies.push(CellDependency::new(
            other_target,
            CellDependencyKind::DependsOn,
            "depends on another target",
        ));
        let mut kernel = FileKernel::open(&path)?;
        append_committed(&mut kernel, first)?;
        append_committed(&mut kernel, support)?;
        append_committed(&mut kernel, second)?;

        let plan = kernel.lookup_plan(&CellLookup {
            dependency_kind: Some(CellDependencyKind::DependsOn),
            ..CellLookup::default()
        });

        assert_eq!(plan.indexed_constraint_count, 1);
        assert_eq!(plan.indexed_constraints, vec!["dependency_kind"]);
        assert_eq!(
            plan.indexed_constraint_plans,
            vec![FileKernelIndexedConstraintPlan {
                name: "dependency_kind",
                candidate_count: 2,
            }]
        );
        assert_eq!(plan.candidate_count, 2);
        assert!(!plan.full_scan);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_filters_by_dependency_target_and_kind() -> Result<(), Box<dyn std::error::Error>>
    {
        let path = temp_kernel_path("continuitydb-file-kernel-dependency");
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
        {
            let mut kernel = FileKernel::open(&path)?;
            dependent = append_committed(&mut kernel, dependent)?;
            append_committed(&mut kernel, unrelated)?;
            append_committed(&mut kernel, support)?;
        }

        let reopened = FileKernel::open(&path)?;
        let results = reopened.lookup_cells(CellLookup {
            dependency_target: Some(target),
            dependency_kind: Some(CellDependencyKind::DependsOn),
            ..CellLookup::default()
        })?;

        assert_eq!(results, vec![dependent]);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_filters_by_dependency_kind_without_target(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-dependency-kind");
        let target = StateCellId::new();
        let other_target = StateCellId::new();
        let mut first = sample_cell("project:continuitydb:dependency-kind-first", 0.9, 12)?;
        first.dependencies.push(CellDependency::new(
            target,
            CellDependencyKind::DependsOn,
            "depends on target",
        ));
        let mut support = sample_cell("project:continuitydb:dependency-kind-support", 0.9, 12)?;
        support.dependencies.push(CellDependency::new(
            target,
            CellDependencyKind::Supports,
            "supports target",
        ));
        let mut second = sample_cell("project:continuitydb:dependency-kind-second", 0.9, 12)?;
        second.dependencies.push(CellDependency::new(
            other_target,
            CellDependencyKind::DependsOn,
            "depends on another target",
        ));
        {
            let mut kernel = FileKernel::open(&path)?;
            first = append_committed(&mut kernel, first)?;
            append_committed(&mut kernel, support)?;
            second = append_committed(&mut kernel, second)?;
        }

        let reopened = FileKernel::open(&path)?;
        let results = reopened.lookup_cells(CellLookup {
            dependency_kind: Some(CellDependencyKind::DependsOn),
            ..CellLookup::default()
        })?;

        assert_eq!(results, vec![first, second]);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rebuilds_dependency_indexes() -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-dependency-index-rebuild");
        let target = StateCellId::new();
        let other_target = StateCellId::new();
        let mut dependent = sample_cell("project:continuitydb:index-dependent", 0.9, 12)?;
        dependent.dependencies.push(CellDependency::new(
            target,
            CellDependencyKind::DependsOn,
            "depends on target",
        ));
        let mut support = sample_cell("project:continuitydb:index-support", 0.9, 12)?;
        support.dependencies.push(CellDependency::new(
            target,
            CellDependencyKind::Supports,
            "supports target",
        ));
        let mut unrelated = sample_cell("project:continuitydb:index-unrelated", 0.9, 12)?;
        unrelated.dependencies.push(CellDependency::new(
            other_target,
            CellDependencyKind::DependsOn,
            "depends on other target",
        ));
        {
            let mut kernel = FileKernel::open(&path)?;
            dependent = append_committed(&mut kernel, dependent)?;
            support = append_committed(&mut kernel, support)?;
            append_committed(&mut kernel, unrelated)?;
        }

        let reopened = FileKernel::open(&path)?;
        let target_positions = reopened
            .index
            .dependency_targets
            .get(&target)
            .cloned()
            .unwrap_or_default();
        let target_indexed = target_positions
            .iter()
            .map(|position| reopened.index.cells[*position].clone())
            .collect::<Vec<_>>();
        let kind_positions = reopened
            .index
            .dependency_target_kinds
            .get(&(target, CellDependencyKind::DependsOn))
            .cloned()
            .unwrap_or_default();
        let kind_indexed = kind_positions
            .iter()
            .map(|position| reopened.index.cells[*position].clone())
            .collect::<Vec<_>>();

        assert_eq!(target_indexed, vec![dependent.clone(), support]);
        assert_eq!(kind_indexed, vec![dependent]);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_updates_dependency_indexes_after_append(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-dependency-index-append");
        let target = StateCellId::new();
        let mut dependent = sample_cell("project:continuitydb:index-append-dependent", 0.9, 12)?;
        dependent.dependencies.push(CellDependency::new(
            target,
            CellDependencyKind::DependsOn,
            "depends on target",
        ));
        let mut kernel = FileKernel::open(&path)?;

        dependent = append_committed(&mut kernel, dependent)?;
        let target_positions = kernel
            .index
            .dependency_targets
            .get(&target)
            .cloned()
            .unwrap_or_default();
        let target_indexed = target_positions
            .iter()
            .map(|position| kernel.index.cells[*position].clone())
            .collect::<Vec<_>>();
        let kind_positions = kernel
            .index
            .dependency_target_kinds
            .get(&(target, CellDependencyKind::DependsOn))
            .cloned()
            .unwrap_or_default();
        let kind_indexed = kind_positions
            .iter()
            .map(|position| kernel.index.cells[*position].clone())
            .collect::<Vec<_>>();

        assert_eq!(target_indexed, vec![dependent.clone()]);
        assert_eq!(kind_indexed, vec![dependent]);
        fs::remove_file(path)?;
        Ok(())
    }
}
