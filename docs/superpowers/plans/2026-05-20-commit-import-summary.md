# Commit Import Summary Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Return backup-page cursor metadata from successful commit imports so embedders and CLI operators can checkpoint incremental sync imports.

**Architecture:** Add summary-returning import APIs that wrap the existing validation and append path. Preserve existing count-returning methods by delegating to the new summary methods, and include `next_after` in CLI non-dry-run import output.

**Tech Stack:** Rust, `continuitydb-api`, `continuitydb-cli`, assert_cmd, serde_json.

---

## File Structure

- Modify `crates/continuitydb-api/src/lib.rs`: add `CommitImportSummary`, summary-returning import methods, and API tests.
- Modify `crates/continuitydb-cli/src/main.rs`: use the file summary import helper and print `next_after`.
- Modify `crates/continuitydb-cli/tests/cli.rs`: assert import output includes the exported cursor.
- Modify `README.md`: add import cursor summary to current scope.
- Modify `docs/roadmap.md`: add Native API and CLI milestones.

## Task 1: Native API Import Summary

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`

- [ ] **Step 1: Write failing API tests**

Add `CommitImportSummary` to the test module import:

```rust
use super::{
    CommitExportBatch, CommitExportFileSummary, CommitImportSummary, CommitSlice, ContinuityDb,
    ContinuityError,
};
```

Add these tests near existing commit import tests:

```rust
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
        vec![sample_cell("project:continuitydb:summary-second", 0.83, 15)?],
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
fn api_import_commit_batch_count_delegates_to_summary() -> Result<(), Box<dyn std::error::Error>> {
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
        vec![sample_cell("project:continuitydb:file-import-summary", 0.91, 12)?],
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
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p continuitydb-api import_summary --all-features
cargo test -p continuitydb-api delegates_to_summary --all-features
```

Expected: FAIL because `CommitImportSummary`, `import_commit_batch_with_summary`, and `import_commits_json_file_with_summary` do not exist.

- [ ] **Step 3: Implement API summary**

Add near `CommitImportValidation`:

```rust
/// Summary of a successful commit import.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommitImportSummary {
    /// Number of commit slices imported.
    pub imported_commits: usize,
    /// Cursor from the imported export batch.
    pub next_after: Option<CommitId>,
}
```

Replace `import_commit_batch` body with:

```rust
let summary = self.import_commit_batch_with_summary(batch)?;
Ok(summary.imported_commits)
```

Add near `import_commit_batch`:

```rust
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
```

Replace `import_commits_json_file` body with:

```rust
let summary = self.import_commits_json_file_with_summary(input_path)?;
Ok(summary.imported_commits)
```

Add near `import_commits_json_file`:

```rust
/// Imports a versioned JSON commit export envelope from a file and returns cursor metadata.
pub fn import_commits_json_file_with_summary<P: AsRef<Path>>(
    &mut self,
    input_path: P,
) -> Result<CommitImportSummary, ContinuityError> {
    let encoded = fs::read(input_path).map_err(|_error| ContinuityError::CommitExportFileIo)?;
    let batch = Self::decode_commit_export_json(&encoded)?;
    self.import_commit_batch_with_summary(batch)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run:

```bash
cargo test -p continuitydb-api import_summary --all-features
cargo test -p continuitydb-api delegates_to_summary --all-features
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/continuitydb-api/src/lib.rs
git commit -m "feat: add commit import summaries"
```

## Task 2: CLI Import Cursor Output

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`
- Modify: `crates/continuitydb-cli/tests/cli.rs`

- [ ] **Step 1: Write failing CLI test update**

In `cli_exports_and_imports_commit_backup`, after reading `export_json` and `import_json`, add:

```rust
assert_eq!(import_json["next_after"], export_json["next_after"]);
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p continuitydb-cli cli_exports_and_imports_commit_backup --all-features
```

Expected: FAIL because import output does not include `next_after`.

- [ ] **Step 3: Implement CLI output**

In `crates/continuitydb-cli/src/main.rs`, replace non-dry-run import code with:

```rust
let summary = db.import_commits_json_file_with_summary(&input_path)?;
serde_json::json!({
    "path": store_path.display().to_string(),
    "input": input_path.display().to_string(),
    "imported_commits": summary.imported_commits,
    "next_after": summary.next_after,
})
```

- [ ] **Step 4: Run test to verify it passes**

Run:

```bash
cargo test -p continuitydb-cli cli_exports_and_imports_commit_backup --all-features
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs
git commit -m "feat: report cli import cursors"
```

## Task 3: Docs and Full Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [ ] **Step 1: Update docs**

Add this README current-scope bullet near backup/import:

```markdown
- Commit import summaries with cursor metadata for checkpointed sync.
```

Add this Native API milestone after commit import dry-run validation:

```markdown
19. Add commit import summaries. Implemented summary-returning import APIs so embedders can retrieve imported counts and backup cursors without decoding envelopes separately.
```

Add this CLI milestone after incremental commit export:

```markdown
12. Report commit import cursors. Extended `continuitydb import-commits` output with `next_after` so operators can checkpoint imported backup pages.
```

- [ ] **Step 2: Run full verification gate**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

Expected: all commands exit 0.

- [ ] **Step 3: Commit docs**

```bash
git add README.md docs/roadmap.md
git commit -m "docs: record commit import summaries"
```
