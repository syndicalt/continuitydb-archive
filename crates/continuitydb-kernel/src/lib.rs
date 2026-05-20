//! Storage kernel interface for ContinuityDB backends.

use chrono::{DateTime, Utc};
use continuitydb_core::{Scope, StateCell};
use std::{
    fs::{File, OpenOptions},
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
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CellLookup {
    /// Optional semantic anchor filter.
    pub semantic_anchor: Option<String>,
    /// Optional scope filter.
    pub scope: Option<Scope>,
    /// Optional valid-time as-of filter.
    pub valid_at: Option<DateTime<Utc>>,
}

/// Minimal append and lookup contract required by the first ContinuityDB milestone.
pub trait StorageKernel {
    /// Appends an immutable StateCell version.
    fn append_cell(&mut self, cell: StateCell) -> Result<(), KernelError>;

    /// Looks up StateCells matching deterministic constraints.
    fn lookup_cells(&self, lookup: CellLookup) -> Result<Vec<StateCell>, KernelError>;
}

/// Append-only JSONL file-backed storage kernel.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileKernel {
    path: PathBuf,
}

impl FileKernel {
    /// Opens an append-only JSONL kernel at the supplied path.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, KernelError> {
        let path = path.as_ref().to_path_buf();
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|_error| KernelError::StoreIo)?;

        Ok(Self { path })
    }

    /// Returns the backing file path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    fn read_cells(&self) -> Result<Vec<StateCell>, KernelError> {
        let file = File::open(&self.path).map_err(|_error| KernelError::StoreIo)?;
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
}

impl StorageKernel for FileKernel {
    fn append_cell(&mut self, cell: StateCell) -> Result<(), KernelError> {
        if self.read_cells()?.iter().any(|stored| stored.id == cell.id) {
            return Err(KernelError::DuplicateCell);
        }

        let encoded = serde_json::to_string(&cell).map_err(|_error| KernelError::StoreCorrupt)?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|_error| KernelError::StoreIo)?;
        file.write_all(encoded.as_bytes())
            .and_then(|()| file.write_all(b"\n"))
            .map_err(|_error| KernelError::StoreIo)
    }

    fn lookup_cells(&self, lookup: CellLookup) -> Result<Vec<StateCell>, KernelError> {
        let cells = self
            .read_cells()?
            .into_iter()
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
            .collect();

        Ok(cells)
    }
}

#[cfg(test)]
mod tests {
    use super::{CellLookup, FileKernel, KernelError, StorageKernel};
    use chrono::{TimeZone, Utc};
    use continuitydb_core::{
        Answerability, CellCost, CellPayload, Citation, Confidence, Evidence, Scope,
        SemanticAnchor, SourceId, StateCell, StateCellId, TrustSignal, ValidTimeRange,
    };
    use std::{fs, path::PathBuf};

    fn temp_kernel_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("{name}-{:?}.jsonl", StateCellId::new()))
    }

    fn sample_cell(
        anchor: &str,
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
                source: SourceId::new("test"),
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
        {
            let mut kernel = FileKernel::open(&path)?;
            kernel.append_cell(cell.clone())?;
        }

        let reopened = FileKernel::open(&path)?;
        let results = reopened.lookup_cells(CellLookup {
            semantic_anchor: Some("project:continuitydb:durable".to_string()),
            ..CellLookup::default()
        })?;

        assert_eq!(results, vec![cell]);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rejects_duplicate_after_reopen() -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-duplicate");
        let cell = sample_cell("project:continuitydb:duplicate", 0.91, 12)?;
        {
            let mut kernel = FileKernel::open(&path)?;
            kernel.append_cell(cell.clone())?;
        }

        let mut reopened = FileKernel::open(&path)?;
        let result = reopened.append_cell(cell);

        assert!(matches!(result, Err(KernelError::DuplicateCell)));
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rejects_corrupt_jsonl() -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-corrupt");
        fs::write(&path, "{not valid json}\n")?;
        let kernel = FileKernel::open(&path)?;

        let result = kernel.lookup_cells(CellLookup::default());

        assert!(matches!(result, Err(KernelError::StoreCorrupt)));
        fs::remove_file(path)?;
        Ok(())
    }
}
