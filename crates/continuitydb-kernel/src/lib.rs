//! Storage kernel interface for ContinuityDB backends.

use chrono::{DateTime, Utc};
use continuitydb_core::{
    ActivationState, CellDependencyKind, CommitId, CommitManifest, Confidence, Scope, StateCell,
    StateCellId, SystemTimeRange,
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

/// Minimal append and lookup contract required by the first ContinuityDB milestone.
pub trait StorageKernel {
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
}

#[derive(Clone, Debug, Default, PartialEq)]
struct FileKernelLog {
    cells: Vec<StateCell>,
    explicit_manifests: Vec<CommitManifest>,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct FileKernelIndex {
    cells: Vec<StateCell>,
    ids: HashMap<StateCellId, usize>,
    anchors: HashMap<String, Vec<usize>>,
    commits: HashMap<CommitId, Vec<usize>>,
    manifests: HashMap<CommitId, CommitManifest>,
    manifest_order: Vec<CommitId>,
}

impl FileKernelIndex {
    fn rebuild(log: FileKernelLog) -> Result<Self, KernelError> {
        let mut index = Self::default();
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
}

/// Append-only JSONL file-backed storage kernel.
#[derive(Clone, Debug)]
pub struct FileKernel {
    path: PathBuf,
    index: FileKernelIndex,
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
        let index = FileKernelIndex::rebuild(log)?;

        Ok(Self { path, index })
    }

    /// Returns the backing file path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Rewrites the backing JSONL log into the current canonical record format.
    pub fn compact(&mut self) -> Result<(), KernelError> {
        let manifests = self.index.list_manifests();
        let encoded = encode_canonical_log(self.index.cells.iter(), manifests.iter())?;
        let temp_path = compact_temp_path(&self.path);
        {
            let mut temp_file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&temp_path)
                .map_err(|_error| KernelError::StoreIo)?;
            temp_file
                .write_all(encoded.as_bytes())
                .map_err(|_error| KernelError::StoreIo)?;
            temp_file.flush().map_err(|_error| KernelError::StoreIo)?;
        }

        let compacted_log = read_log_from_path(&temp_path)?;
        let compacted_index = FileKernelIndex::rebuild(compacted_log)?;
        fs::rename(&temp_path, &self.path).map_err(|_error| KernelError::StoreIo)?;
        self.index = compacted_index;
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
    writeln!(file, "{encoded}").map_err(|_error| KernelError::StoreIo)
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
                seen_header = true;
            }
            Ok(FileKernelRecord::Cell { cell, checksum }) => {
                seen_data = true;
                validate_file_record_checksum(cell.as_ref(), checksum.as_deref())
                    .map_err(|_error| corrupt_record(line_number))?;
                log.cells.push(*cell);
            }
            Ok(FileKernelRecord::Commit { manifest, checksum }) => {
                seen_data = true;
                validate_file_record_checksum(&manifest, checksum.as_deref())
                    .map_err(|_error| corrupt_record(line_number))?;
                log.explicit_manifests.push(manifest);
            }
            Err(_record_error) => {
                seen_data = true;
                log.cells.push(
                    serde_json::from_str(&line)
                        .map_err(|_cell_error| corrupt_record(line_number))?,
                );
            }
        }
    }

    Ok(log)
}

impl StorageKernel for FileKernel {
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
        file.write_all(encoded.as_bytes())
            .map_err(|_error| KernelError::StoreIo)?;

        for cell in stamped {
            self.index.insert(cell)?;
        }
        self.index.apply_explicit_manifest(manifest)?;
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
}

#[cfg(test)]
mod tests {
    use super::{CellLookup, CommitManifestLookup, FileKernel, KernelError, StorageKernel};
    use chrono::{TimeZone, Utc};
    use continuitydb_core::{
        ActivationState, Answerability, CellCost, CellDependency, CellDependencyKind, CellPayload,
        Citation, CommitId, Confidence, Evidence, Scope, SemanticAnchor, SourceId, StateCell,
        StateCellId, TrustSignal, ValidTimeRange,
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
}
