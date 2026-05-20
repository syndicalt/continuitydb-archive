# Commit Import Dry Run Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add non-mutating commit import validation for embedders and CLI operators.

**Architecture:** Reuse the existing internal import validation logic in `continuitydb-api`, expose explicit validation summaries, and route CLI `import-commits --dry-run` through the native file helper. Real import behavior remains unchanged.

**Tech Stack:** Rust, existing `continuitydb-api` and `continuitydb-cli` crates, serde_json, clap, assert_cmd tests.

---

## File Structure

- Modify `crates/continuitydb-api/src/lib.rs`: add `CommitImportValidation`, public validation methods, and API tests.
- Modify `crates/continuitydb-cli/src/main.rs`: add `import-commits --dry-run` and output JSON.
- Modify `crates/continuitydb-cli/tests/cli.rs`: add CLI dry-run tests.
- Modify `README.md`: add commit import dry-run validation to current scope.
- Modify `docs/roadmap.md`: add Native API and CLI milestones.

## Task 1: Native API Import Validation

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`

- [ ] **Step 1: Write failing API tests**

Add these tests near existing commit import tests:

```rust
#[test]
fn api_validates_commit_import_without_mutation() -> Result<(), Box<dyn std::error::Error>> {
    let committed_at = Utc
        .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let commit_id = CommitId::new();
    let mut source = ContinuityDb::new(MemoryKernel::default());
    source.ingest_cells_at_with_commit_id(
        vec![sample_cell("project:continuitydb:dry-run-source", 0.91, 12)?],
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
fn api_validate_commit_import_reports_duplicate_commit() -> Result<(), Box<dyn std::error::Error>> {
    let committed_at = Utc
        .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let commit_id = CommitId::new();
    let mut source = ContinuityDb::new(MemoryKernel::default());
    source.ingest_cells_at_with_commit_id(
        vec![sample_cell("project:continuitydb:dry-run-duplicate", 0.91, 12)?],
        committed_at,
        commit_id,
    )?;
    let batch = source.export_commits(CommitManifestLookup::default())?;
    let mut target = ContinuityDb::new(MemoryKernel::default());
    target.ingest_cells_at_with_commit_id(
        vec![sample_cell("project:continuitydb:dry-run-existing", 0.83, 15)?],
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
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p continuitydb-api dry_run --all-features
```

Expected: FAIL because validation methods and summary do not exist.

- [ ] **Step 3: Implement native validation summary and methods**

Add near `CommitExportFileSummary`:

```rust
/// Summary of a non-mutating commit import validation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommitImportValidation {
    /// Number of commit slices that would be imported.
    pub valid_commits: usize,
}
```

Add to generic `impl<K: StorageKernel> ContinuityDb<K>` near `import_commit_batch`:

```rust
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
```

Update `import_commit_batch` to call the public method:

```rust
self.validate_commit_import(&batch)?;
```

Add to `impl ContinuityDb<FileKernel>`:

```rust
/// Validates a versioned JSON commit export envelope from a file without mutating the store.
pub fn validate_commits_json_file<P: AsRef<Path>>(
    &self,
    input_path: P,
) -> Result<CommitImportValidation, ContinuityError> {
    let encoded = fs::read(input_path).map_err(|_error| ContinuityError::CommitExportFileIo)?;
    let batch = Self::decode_commit_export_json(&encoded)?;
    self.validate_commit_import(&batch)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run:

```bash
cargo test -p continuitydb-api dry_run --all-features
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/continuitydb-api/src/lib.rs
git commit -m "feat: add commit import dry run api"
```

## Task 2: CLI Import Dry Run

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`
- Modify: `crates/continuitydb-cli/tests/cli.rs`

- [ ] **Step 1: Write failing CLI tests**

Add these tests near backup/import CLI tests:

```rust
#[test]
fn cli_import_commits_dry_run_validates_without_mutation(
) -> Result<(), Box<dyn std::error::Error>> {
    let source_path = temp_store_path("continuitydb-cli-dry-run-source");
    let target_path = temp_store_path("continuitydb-cli-dry-run-target");
    let backup_path = temp_store_path("continuitydb-cli-dry-run-backup");
    write_committed_store(&source_path, "project:continuitydb:cli-dry-run")?;
    Command::cargo_bin("continuitydb")?
        .arg("export-commits")
        .arg(&source_path)
        .arg(&backup_path)
        .assert()
        .success();

    let output = Command::cargo_bin("continuitydb")?
        .arg("import-commits")
        .arg(&target_path)
        .arg(&backup_path)
        .arg("--dry-run")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;
    let target_batch = ContinuityDb::new(FileKernel::open(&target_path)?)
        .export_commits(CommitManifestLookup::default())?;

    assert_eq!(json["dry_run"].as_bool(), Some(true));
    assert_eq!(json["valid_commits"].as_u64(), Some(1));
    assert_eq!(target_batch.slices.len(), 0);

    fs::remove_file(source_path)?;
    fs::remove_file(target_path)?;
    fs::remove_file(backup_path)?;
    Ok(())
}

#[test]
fn cli_import_commits_dry_run_fails_for_invalid_envelope(
) -> Result<(), Box<dyn std::error::Error>> {
    let target_path = temp_store_path("continuitydb-cli-dry-run-invalid-target");
    let backup_path = temp_store_path("continuitydb-cli-dry-run-invalid-backup");
    fs::write(&backup_path, "{not valid json}\n")?;

    Command::cargo_bin("continuitydb")?
        .arg("import-commits")
        .arg(&target_path)
        .arg(&backup_path)
        .arg("--dry-run")
        .assert()
        .failure();

    let _ = fs::remove_file(target_path);
    fs::remove_file(backup_path)?;
    Ok(())
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p continuitydb-cli dry_run --all-features
```

Expected: FAIL because `import-commits --dry-run` is not accepted.

- [ ] **Step 3: Implement CLI flag and output**

Update `ImportCommits`:

```rust
ImportCommits {
    /// Path to the JSONL file-backed store.
    store_path: PathBuf,
    /// Path to read the versioned JSON commit export envelope from.
    input_path: PathBuf,
    /// Validate the import without mutating the target store.
    #[arg(long = "dry-run")]
    dry_run: bool,
},
```

Update match arm:

```rust
Some(Command::ImportCommits {
    store_path,
    input_path,
    dry_run,
}) => {
    let mut db = open_file_database(&store_path)?;
    let output = if dry_run {
        let validation = db.validate_commits_json_file(&input_path)?;
        serde_json::json!({
            "path": store_path.display().to_string(),
            "input": input_path.display().to_string(),
            "dry_run": true,
            "valid_commits": validation.valid_commits,
        })
    } else {
        let imported_commits = db.import_commits_json_file(&input_path)?;
        serde_json::json!({
            "path": store_path.display().to_string(),
            "input": input_path.display().to_string(),
            "imported_commits": imported_commits,
        })
    };
    println!("{}", serde_json::to_string_pretty(&output)?);
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run:

```bash
cargo test -p continuitydb-cli dry_run --all-features
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs
git commit -m "feat: add cli commit import dry run"
```

## Task 3: Docs and Full Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [ ] **Step 1: Update docs**

Add this README current-scope bullet near commit backup/import:

```markdown
- Commit import dry-run validation for backup and sync workflows.
```

Add these roadmap milestones:

Native API:

```markdown
18. Add commit import dry-run validation. Implemented `validate_commit_import` and `validate_commits_json_file` so embedders can validate replay batches and backup files without mutating target stores.
```

CLI:

```markdown
10. Add commit import dry-run validation. Implemented `continuitydb import-commits --dry-run` so operators can validate backup files against a target store before mutation.
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
git commit -m "docs: record commit import dry run"
```

