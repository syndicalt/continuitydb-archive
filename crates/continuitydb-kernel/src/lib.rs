//! Storage kernel interface for ContinuityDB backends.

use chrono::{DateTime, Utc};
use continuitydb_core::{
    ActivationState, CellDependencyKind, Confidence, Scope, StateCell, StateCellId, SystemTimeRange,
};
use std::{
    collections::HashMap,
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
};
use thiserror::Error;

/// Errors produced by storage kernels.
#[derive(Debug, Error, PartialEq)]
pub enum KernelError {
    /// A duplicate immutable StateCell version was appended.
    #[error("state cell already exists")]
    DuplicateCell,
    /// Storage kernel I/O failed.
    #[error("storage kernel I/O failed")]
    StoreIo,
    /// Storage kernel content could not be decoded.
    #[error("storage kernel content is corrupt")]
    StoreCorrupt,
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
    ) -> Result<(), KernelError>;

    /// Looks up StateCells matching deterministic constraints.
    fn lookup_cells(&self, lookup: CellLookup) -> Result<Vec<StateCell>, KernelError>;
}

#[derive(Clone, Debug, Default, PartialEq)]
struct FileKernelIndex {
    cells: Vec<StateCell>,
    ids: HashMap<StateCellId, usize>,
    anchors: HashMap<String, Vec<usize>>,
}

impl FileKernelIndex {
    fn rebuild(cells: Vec<StateCell>) -> Result<Self, KernelError> {
        let mut index = Self::default();
        for cell in cells {
            index.insert(cell)?;
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
        self.cells.push(cell);
        Ok(())
    }

    fn contains_id(&self, id: StateCellId) -> bool {
        self.ids.contains_key(&id)
    }

    fn position_by_id(&self, id: StateCellId) -> Option<usize> {
        self.ids.get(&id).copied()
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

        let cells = read_cells_from_path(&path)?;
        let index = FileKernelIndex::rebuild(cells)?;

        Ok(Self { path, index })
    }

    /// Returns the backing file path.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

fn read_cells_from_path(path: &Path) -> Result<Vec<StateCell>, KernelError> {
    let file = File::open(path).map_err(|_error| KernelError::StoreIo)?;
    let reader = BufReader::new(file);
    let mut cells = Vec::new();

    for line in reader.lines() {
        let line = line.map_err(|_error| KernelError::StoreIo)?;
        if line.trim().is_empty() {
            continue;
        }

        cells.push(serde_json::from_str(&line).map_err(|_error| KernelError::StoreCorrupt)?);
    }

    Ok(cells)
}

impl StorageKernel for FileKernel {
    fn append_cell_at(
        &mut self,
        mut cell: StateCell,
        committed_at: DateTime<Utc>,
    ) -> Result<(), KernelError> {
        if self.index.contains_id(cell.id) {
            return Err(KernelError::DuplicateCell);
        }

        cell.system_time = SystemTimeRange::open_from(committed_at);
        let encoded = serde_json::to_string(&cell).map_err(|_error| KernelError::StoreCorrupt)?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|_error| KernelError::StoreIo)?;
        file.write_all(encoded.as_bytes())
            .and_then(|()| file.write_all(b"\n"))
            .map_err(|_error| KernelError::StoreIo)?;

        self.index.insert(cell)
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
    use super::{CellLookup, FileKernel, KernelError, StorageKernel};
    use chrono::{TimeZone, Utc};
    use continuitydb_core::{
        ActivationState, Answerability, CellCost, CellDependency, CellDependencyKind, CellPayload,
        Citation, Confidence, Evidence, Scope, SemanticAnchor, SourceId, StateCell, StateCellId,
        TrustSignal, ValidTimeRange,
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
        kernel.append_cell_at(cell.clone(), committed_at)?;
        cell.system_time = continuitydb_core::SystemTimeRange::open_from(committed_at);
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

        assert!(matches!(result, Err(KernelError::StoreCorrupt)));
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
