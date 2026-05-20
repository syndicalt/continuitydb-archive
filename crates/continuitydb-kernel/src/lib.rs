//! Storage kernel interface for ContinuityDB backends.

use chrono::{DateTime, Utc};
use continuitydb_core::{
    ActivationState, CellDependencyKind, CommitId, CommitManifest, Confidence, RevisionLinkKind,
    RevisionLinkRecord, Scope, StateCell, StateCellId, SystemTimeRange,
};
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Write as IoWrite},
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
    /// Optional exact answerability question filter.
    pub answerability_question: Option<String>,
    /// Optional exact evidence-source filter.
    pub evidence_source: Option<String>,
    /// Optional minimum evidence confidence filter.
    pub minimum_confidence: Option<Confidence>,
    /// Optional dependency target filter.
    pub dependency_target: Option<StateCellId>,
    /// Optional dependency kind filter, applied with dependency target when present.
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
            persistent_indexes: false,
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
const FILE_KERNEL_CHECKSUM_ALGORITHM: &str = "continuitydb-fnv1a64";
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
    answerability_questions: HashMap<String, Vec<usize>>,
    evidence_sources: HashMap<String, Vec<usize>>,
    activations: HashMap<ActivationState, Vec<usize>>,
    dependency_targets: HashMap<StateCellId, Vec<usize>>,
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
            index.insert_revision_link(revision_link);
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
        self.activations
            .entry(cell.activation)
            .or_default()
            .push(position);
        for dependency in &cell.dependencies {
            self.dependency_targets
                .entry(dependency.target)
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

    fn insert_revision_link(&mut self, revision_link: RevisionLinkRecord) {
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

    fn manifest_by_id(&self, commit_id: CommitId) -> Option<CommitManifest> {
        self.manifests.get(&commit_id).cloned()
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

    fn list_revision_links(&self, lookup: RevisionLinkLookup) -> Vec<RevisionLinkRecord> {
        let candidates: Vec<&RevisionLinkRecord> = if let (Some(source), Some(target), Some(kind)) =
            (lookup.source, lookup.target, lookup.kind)
        {
            self.revision_link_source_target_kinds
                .get(&(source, target, kind))
                .map(|positions| {
                    positions
                        .iter()
                        .map(|position| &self.revision_links[*position])
                        .collect()
                })
                .unwrap_or_default()
        } else if let (Some(source), Some(target)) = (lookup.source, lookup.target) {
            self.revision_link_source_targets
                .get(&(source, target))
                .map(|positions| {
                    positions
                        .iter()
                        .map(|position| &self.revision_links[*position])
                        .collect()
                })
                .unwrap_or_default()
        } else if let (Some(source), Some(kind)) = (lookup.source, lookup.kind) {
            self.revision_link_source_kinds
                .get(&(source, kind))
                .map(|positions| {
                    positions
                        .iter()
                        .map(|position| &self.revision_links[*position])
                        .collect()
                })
                .unwrap_or_default()
        } else if let (Some(target), Some(kind)) = (lookup.target, lookup.kind) {
            self.revision_link_target_kinds
                .get(&(target, kind))
                .map(|positions| {
                    positions
                        .iter()
                        .map(|position| &self.revision_links[*position])
                        .collect()
                })
                .unwrap_or_default()
        } else if let Some(source) = lookup.source {
            self.revision_link_sources
                .get(&source)
                .map(|positions| {
                    positions
                        .iter()
                        .map(|position| &self.revision_links[*position])
                        .collect()
                })
                .unwrap_or_default()
        } else if let Some(target) = lookup.target {
            self.revision_link_targets
                .get(&target)
                .map(|positions| {
                    positions
                        .iter()
                        .map(|position| &self.revision_links[*position])
                        .collect()
                })
                .unwrap_or_default()
        } else if let Some(kind) = lookup.kind {
            self.revision_link_kinds
                .get(&kind)
                .map(|positions| {
                    positions
                        .iter()
                        .map(|position| &self.revision_links[*position])
                        .collect()
                })
                .unwrap_or_default()
        } else {
            self.revision_links.iter().collect()
        };

        candidates
            .into_iter()
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
            .collect()
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
    /// Current durable file size in bytes.
    pub file_size_bytes: u64,
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
        }
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
        let health = log.health;
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
        Ok(FileKernelStatus {
            cell_count: self.index.cells.len(),
            commit_count: self.index.manifest_order.len(),
            file_size_bytes,
        })
    }

    /// Returns the file-format health report captured for this store.
    pub fn health(&self) -> FileKernelHealth {
        self.health
    }

    /// Rewrites the backing JSONL log into the current canonical record format.
    pub fn compact(&mut self) -> Result<(), KernelError> {
        let manifests = self.index.list_manifests();
        let encoded = encode_canonical_log(
            self.index.cells.iter(),
            manifests.iter(),
            self.index.revision_links.iter(),
        )?;
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
        self.health = compacted_health;
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
            if self.index.contains_id(cell.id) || !batch_ids.insert(cell.id) {
                return Err(KernelError::DuplicateCell);
            }

            cell.system_time = SystemTimeRange::open_from(committed_at);
            cell.commit_id = commit_id;
            stamped.push(cell);
        }

        if stamped.is_empty() {
            return Ok(());
        }

        if self.index.contains_commit(commit_id) {
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
        Ok(())
    }

    fn lookup_cells(&self, lookup: CellLookup) -> Result<Vec<StateCell>, KernelError> {
        let candidates: Vec<&StateCell> = if let Some(cell_id) = lookup.cell_id {
            self.index
                .position_by_id(cell_id)
                .map(|position| vec![&self.index.cells[position]])
                .unwrap_or_default()
        } else if let Some(anchor) = lookup.semantic_anchor.as_ref() {
            self.index
                .anchors
                .get(anchor)
                .map(|positions| {
                    positions
                        .iter()
                        .map(|position| &self.index.cells[*position])
                        .collect()
                })
                .unwrap_or_default()
        } else if let Some(commit_id) = lookup.commit_id {
            self.index
                .commits
                .get(&commit_id)
                .map(|positions| {
                    positions
                        .iter()
                        .map(|position| &self.index.cells[*position])
                        .collect()
                })
                .unwrap_or_default()
        } else if let Some(question) = lookup.answerability_question.as_ref() {
            self.index
                .answerability_questions
                .get(question)
                .map(|positions| {
                    positions
                        .iter()
                        .map(|position| &self.index.cells[*position])
                        .collect()
                })
                .unwrap_or_default()
        } else if let Some(source) = lookup.evidence_source.as_ref() {
            self.index
                .evidence_sources
                .get(source)
                .map(|positions| {
                    positions
                        .iter()
                        .map(|position| &self.index.cells[*position])
                        .collect()
                })
                .unwrap_or_default()
        } else if let Some(activation) = lookup.activation {
            self.index
                .activations
                .get(&activation)
                .map(|positions| {
                    positions
                        .iter()
                        .map(|position| &self.index.cells[*position])
                        .collect()
                })
                .unwrap_or_default()
        } else if let Some(target) = lookup.dependency_target {
            if let Some(kind) = lookup.dependency_kind {
                self.index
                    .dependency_target_kinds
                    .get(&(target, kind))
                    .map(|positions| {
                        positions
                            .iter()
                            .map(|position| &self.index.cells[*position])
                            .collect()
                    })
                    .unwrap_or_default()
            } else {
                self.index
                    .dependency_targets
                    .get(&target)
                    .map(|positions| {
                        positions
                            .iter()
                            .map(|position| &self.index.cells[*position])
                            .collect()
                    })
                    .unwrap_or_default()
            }
        } else {
            self.index.cells.iter().collect()
        };

        let cells = candidates
            .into_iter()
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
        Ok(self.index.manifest_by_id(commit_id))
    }

    fn list_commit_manifests(&self) -> Result<Vec<CommitManifest>, KernelError> {
        Ok(self.index.list_manifests())
    }

    fn list_commit_manifests_matching(
        &self,
        lookup: CommitManifestLookup,
    ) -> Result<Vec<CommitManifest>, KernelError> {
        self.index.list_manifests_matching(lookup)
    }

    fn append_revision_link(
        &mut self,
        revision_link: RevisionLinkRecord,
    ) -> Result<(), KernelError> {
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

        self.index.insert_revision_link(revision_link);
        self.health.canonical_records += 1;
        Ok(())
    }

    fn list_revision_links(
        &self,
        lookup: RevisionLinkLookup,
    ) -> Result<Vec<RevisionLinkRecord>, KernelError> {
        Ok(self.index.list_revision_links(lookup))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        sync_parent_directory, write_all_durable, CellLookup, CommitManifestLookup, FileKernel,
        KernelCapabilities, KernelDurability, KernelError, KernelRequirements, RevisionLinkLookup,
        StorageKernel,
    };
    use chrono::{TimeZone, Utc};
    use continuitydb_core::{
        ActivationState, Answerability, CellCost, CellDependency, CellDependencyKind, CellPayload,
        Citation, CommitId, Confidence, Evidence, RevisionLinkKind, RevisionLinkRecord, Scope,
        SemanticAnchor, SourceId, StateCell, StateCellId, TrustSignal, ValidTimeRange,
    };
    use std::{fs, path::PathBuf};

    fn temp_kernel_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("{name}-{:?}.jsonl", StateCellId::new()))
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
        assert!(!capabilities.persistent_indexes);
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
        assert!(status.file_size_bytes > 0);

        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_status_reports_visible_cells_and_commits(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-status-populated");
        let committed_at = test_commit_time()?;
        let commit_id = CommitId::new();
        let first = sample_cell("project:continuitydb:status-first", 0.91, 12)?;
        let second = sample_cell("project:continuitydb:status-second", 0.83, 15)?;
        let mut kernel = FileKernel::open(&path)?;
        kernel.append_cells_at_with_commit_id(vec![first, second], committed_at, commit_id)?;

        let status = kernel.status()?;

        assert_eq!(status.cell_count, 2);
        assert_eq!(status.commit_count, 1);
        assert!(status.file_size_bytes > 0);

        fs::remove_file(path)?;
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
    fn file_append_log_capabilities_do_not_satisfy_indexed_embedded_requirements() {
        assert!(!KernelCapabilities::file_append_log()
            .satisfies(KernelRequirements::indexed_embedded()));
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
