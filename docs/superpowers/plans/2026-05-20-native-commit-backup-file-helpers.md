# Native Commit Backup File Helpers Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add native `ContinuityDb<FileKernel>` helpers for writing and reading versioned commit backup envelope files.

**Architecture:** Add a deterministic summary type and two file-backed helper methods in `continuitydb-api`. The helpers compose existing commit export/import and JSON envelope APIs with `std::fs` read/write. Refactor the CLI backup/restore commands to call the native helpers.

**Tech Stack:** Rust, std::fs, existing `continuitydb-api`, `continuitydb-kernel`, `continuitydb-cli`, serde_json tests.

---

## File Structure

- Modify `crates/continuitydb-api/src/lib.rs`: add `CommitExportFileSummary`, `ContinuityError::CommitExportFileIo`, file helper methods, and API tests.
- Modify `crates/continuitydb-cli/src/main.rs`: replace direct file I/O and envelope orchestration with native helper calls.
- Modify `README.md`: add native commit backup file helpers to current scope.
- Modify `docs/roadmap.md`: add Native API milestone.
- Modify `docs/superpowers/plans/2026-05-20-native-commit-backup-file-helpers.md`: track completed steps.

## Task 1: Failing Native File Helper Tests

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`

- [ ] **Step 1: Add test imports**

Change the test module import:

```rust
use super::{CommitExportBatch, CommitSlice, ContinuityDb, ContinuityError};
```

to:

```rust
use super::{
    CommitExportBatch, CommitExportFileSummary, CommitSlice, ContinuityDb, ContinuityError,
};
```

- [ ] **Step 2: Add export file helper test**

Add near the commit export JSON tests:

```rust
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
```

- [ ] **Step 3: Add import file helper test**

Add:

```rust
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
```

- [ ] **Step 4: Add invalid JSON import file helper test**

Add:

```rust
#[test]
fn api_import_commit_backup_json_file_rejects_invalid_json() -> Result<(), Box<dyn std::error::Error>>
{
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
```

- [ ] **Step 5: Add export file I/O failure test**

Add:

```rust
#[test]
fn api_export_commit_backup_json_file_reports_io_failure() -> Result<(), Box<dyn std::error::Error>>
{
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
        vec![sample_cell("project:continuitydb:file-export-io", 0.91, 12)?],
        committed_at,
        commit_id,
    )?;

    let result = db.export_commits_json_file(CommitManifestLookup::default(), backup_path);

    assert!(matches!(result, Err(ContinuityError::CommitExportFileIo)));

    std::fs::remove_file(source_path)?;
    Ok(())
}
```

- [ ] **Step 6: Run targeted API tests and verify RED**

Run:

```bash
cargo test -p continuitydb-api commit_backup_json_file
```

Expected: compilation fails because `CommitExportFileSummary`, `export_commits_json_file`, `import_commits_json_file`, and `CommitExportFileIo` do not exist yet.

## Task 2: Native File Helper Implementation

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`

- [ ] **Step 1: Add std imports**

Change:

```rust
use std::collections::HashSet;
```

to:

```rust
use std::{collections::HashSet, fs, path::Path};
```

- [ ] **Step 2: Add file I/O error variant**

Add to `ContinuityError` after `CommitExportJson`:

```rust
/// Commit export envelope file could not be read or written.
#[error("commit export envelope file I/O failed")]
CommitExportFileIo,
```

- [ ] **Step 3: Add summary type**

Add after `CommitExportBatch`:

```rust
/// Summary of a commit export envelope written to a file.
#[derive(Clone, Debug, PartialEq)]
pub struct CommitExportFileSummary {
    /// Number of commit slices exported.
    pub exported_commits: usize,
    /// Cursor to use as `CommitManifestLookup.after` for the next export batch.
    pub next_after: Option<CommitId>,
}
```

- [ ] **Step 4: Add FileKernel helper methods**

Extend `impl ContinuityDb<FileKernel>`:

```rust
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
    let encoded = fs::read(input_path).map_err(|_error| ContinuityError::CommitExportFileIo)?;
    let batch = Self::decode_commit_export_json(&encoded)?;
    self.import_commit_batch(batch)
}
```

- [ ] **Step 5: Run targeted API tests and verify GREEN**

Run:

```bash
cargo test -p continuitydb-api commit_backup_json_file
```

Expected: native file helper tests pass.

## Task 3: CLI Refactor and Docs

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-native-commit-backup-file-helpers.md`

- [ ] **Step 1: Refactor CLI imports**

Change:

```rust
use continuitydb_kernel::{CommitManifestLookup, FileKernel, StorageKernel};
use std::{fs, path::PathBuf};
```

to:

```rust
use continuitydb_kernel::{CommitManifestLookup, FileKernel, StorageKernel};
use std::path::PathBuf;
```

- [ ] **Step 2: Refactor export command**

Replace the export arm body with:

```rust
let db = ContinuityDb::new(FileKernel::open(&store_path)?);
let summary = db.export_commits_json_file(CommitManifestLookup::default(), &output_path)?;
let output = serde_json::json!({
    "path": store_path.display().to_string(),
    "output": output_path.display().to_string(),
    "exported_commits": summary.exported_commits,
    "next_after": summary.next_after,
});
println!("{}", serde_json::to_string_pretty(&output)?);
```

- [ ] **Step 3: Refactor import command**

Replace the import arm body with:

```rust
let mut db = ContinuityDb::new(FileKernel::open(&store_path)?);
let imported_commits = db.import_commits_json_file(&input_path)?;
let output = serde_json::json!({
    "path": store_path.display().to_string(),
    "input": input_path.display().to_string(),
    "imported_commits": imported_commits,
});
println!("{}", serde_json::to_string_pretty(&output)?);
```

- [ ] **Step 4: Run targeted CLI tests**

Run:

```bash
cargo test -p continuitydb-cli commit
```

Expected: CLI backup/restore tests still pass through the native helpers.

- [ ] **Step 5: Update README**

Add to Current Scope near the native commit export envelope bullet:

```markdown
- Native commit backup and restore file helper API.
```

- [ ] **Step 6: Update roadmap**

Add a Native API milestone after the JSON envelope milestone:

```markdown
11. Add native commit backup and restore file helpers. Implemented `ContinuityDb<FileKernel>::export_commits_json_file` and `import_commits_json_file` so embedders can write and read versioned commit export envelope files without duplicating CLI file I/O orchestration.
```

## Task 4: Full Verification and Commit

**Files:**
- Modify: `docs/superpowers/plans/2026-05-20-native-commit-backup-file-helpers.md`

- [ ] **Step 1: Run full verification gate**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

Expected: every command exits 0.

- [ ] **Step 2: Commit**

Run:

```bash
git add README.md docs/roadmap.md crates/continuitydb-api/src/lib.rs crates/continuitydb-cli/src/main.rs docs/superpowers/plans/2026-05-20-native-commit-backup-file-helpers.md
git commit -m "feat: add native commit backup file helpers"
```
