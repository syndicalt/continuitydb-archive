# CLI Commit Backup and Restore Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `continuitydb export-commits <store-path> <output-path>` and `continuitydb import-commits <store-path> <input-path>` for file-backed commit backup and restore.

**Architecture:** Keep the CLI thin over existing database APIs. The export command opens `FileKernel`, calls `export_commits`, encodes the versioned JSON envelope, writes it to disk, and prints a JSON summary. The import command opens `FileKernel`, reads and decodes the versioned envelope, imports the validated batch, and prints a JSON summary.

**Tech Stack:** Rust, clap, assert_cmd, serde_json, existing `continuitydb-api`, `continuitydb-kernel`, and `continuitydb-core`.

---

## File Structure

- Modify `crates/continuitydb-cli/src/main.rs`: add `export-commits` and `import-commits` commands.
- Modify `crates/continuitydb-cli/tests/cli.rs`: add end-to-end CLI tests and file-store helpers.
- Modify `README.md`: add CLI commit backup/restore to current scope.
- Modify `docs/roadmap.md`: add CLI milestone.
- Modify `docs/superpowers/plans/2026-05-20-cli-commit-backup-restore.md`: track completed steps.

## Task 1: Failing CLI Backup and Restore Tests

**Files:**
- Modify: `crates/continuitydb-cli/tests/cli.rs`

- [ ] **Step 1: Add test imports**

Add these imports near the existing imports:

```rust
use continuitydb_api::ContinuityDb;
use continuitydb_kernel::{CommitManifestLookup, FileKernel};
```

- [ ] **Step 2: Add committed file-store helper**

Add this helper near `write_legacy_store`:

```rust
fn write_committed_store(
    path: &PathBuf,
    anchor: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let committed_at = Utc
        .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let commit_id = CommitId::new();
    let mut db = ContinuityDb::new(FileKernel::open(path)?);
    db.ingest_cells_at_with_commit_id(vec![test_cell(anchor)?], committed_at, commit_id)?;
    Ok(())
}
```

- [ ] **Step 3: Add export/import roundtrip test**

Add this test near the existing CLI file-store tests:

```rust
#[test]
fn cli_exports_and_imports_commit_backup() -> Result<(), Box<dyn std::error::Error>> {
    let source_path = temp_store_path("continuitydb-cli-export-source");
    let target_path = temp_store_path("continuitydb-cli-import-target");
    let backup_path = temp_store_path("continuitydb-cli-export-backup");
    write_committed_store(&source_path, "project:continuitydb:cli-backup")?;

    let export_output = Command::cargo_bin("continuitydb")?
        .arg("export-commits")
        .arg(&source_path)
        .arg(&backup_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let export_json: Value = serde_json::from_slice(&export_output)?;
    let envelope_json: Value = serde_json::from_slice(&fs::read(&backup_path)?)?;

    assert_eq!(export_json["path"].as_str(), source_path.to_str());
    assert_eq!(export_json["output"].as_str(), backup_path.to_str());
    assert_eq!(export_json["exported_commits"].as_u64(), Some(1));
    assert!(export_json["next_after"].is_null());
    assert_eq!(
        envelope_json["format"].as_str(),
        Some("continuitydb.commit_export")
    );
    assert_eq!(envelope_json["version"].as_u64(), Some(1));

    let import_output = Command::cargo_bin("continuitydb")?
        .arg("import-commits")
        .arg(&target_path)
        .arg(&backup_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let import_json: Value = serde_json::from_slice(&import_output)?;
    let source_batch =
        ContinuityDb::new(FileKernel::open(&source_path)?).export_commits(CommitManifestLookup::default())?;
    let target_batch =
        ContinuityDb::new(FileKernel::open(&target_path)?).export_commits(CommitManifestLookup::default())?;

    assert_eq!(import_json["path"].as_str(), target_path.to_str());
    assert_eq!(import_json["input"].as_str(), backup_path.to_str());
    assert_eq!(import_json["imported_commits"].as_u64(), Some(1));
    assert_eq!(target_batch, source_batch);

    fs::remove_file(source_path)?;
    fs::remove_file(target_path)?;
    fs::remove_file(backup_path)?;
    Ok(())
}
```

- [ ] **Step 4: Add invalid import test**

Add:

```rust
#[test]
fn cli_import_commits_fails_for_invalid_envelope() -> Result<(), Box<dyn std::error::Error>> {
    let target_path = temp_store_path("continuitydb-cli-import-invalid-target");
    let backup_path = temp_store_path("continuitydb-cli-import-invalid-backup");
    fs::write(&backup_path, "{not valid json}\n")?;

    Command::cargo_bin("continuitydb")?
        .arg("import-commits")
        .arg(&target_path)
        .arg(&backup_path)
        .assert()
        .failure();

    let _ = fs::remove_file(target_path);
    fs::remove_file(backup_path)?;
    Ok(())
}
```

- [ ] **Step 5: Run targeted CLI tests and verify RED**

Run:

```bash
cargo test -p continuitydb-cli commit
```

Expected: tests fail because `export-commits` and `import-commits` are not recognized commands.

## Task 2: CLI Command Implementation

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`

- [ ] **Step 1: Add imports**

Change the imports to include `CommitManifestLookup` and `fs`:

```rust
use continuitydb_kernel::{CommitManifestLookup, FileKernel, StorageKernel};
use std::{fs, path::PathBuf};
```

- [ ] **Step 2: Add command variants**

Add these variants to `enum Command` after `CompactFile`:

```rust
/// Export all file-backed commit slices to a versioned JSON backup envelope.
ExportCommits {
    /// Path to the JSONL file-backed store.
    store_path: PathBuf,
    /// Path to write the versioned JSON commit export envelope.
    output_path: PathBuf,
},
/// Import a versioned JSON commit backup envelope into a file-backed store.
ImportCommits {
    /// Path to the JSONL file-backed store.
    store_path: PathBuf,
    /// Path to read the versioned JSON commit export envelope from.
    input_path: PathBuf,
},
```

- [ ] **Step 3: Add match arms**

Add these arms to the `match cli.command` block:

```rust
Some(Command::ExportCommits {
    store_path,
    output_path,
}) => {
    let db = ContinuityDb::new(FileKernel::open(&store_path)?);
    let batch = db.export_commits(CommitManifestLookup::default())?;
    let exported_commits = batch.slices.len();
    let next_after = batch.next_after;
    let encoded = ContinuityDb::<FileKernel>::encode_commit_export_json(batch)?;
    fs::write(&output_path, encoded)?;
    let output = serde_json::json!({
        "path": store_path.display().to_string(),
        "output": output_path.display().to_string(),
        "exported_commits": exported_commits,
        "next_after": next_after,
    });
    println!("{}", serde_json::to_string_pretty(&output)?);
}
Some(Command::ImportCommits {
    store_path,
    input_path,
}) => {
    let encoded = fs::read(&input_path)?;
    let batch = ContinuityDb::<FileKernel>::decode_commit_export_json(&encoded)?;
    let mut db = ContinuityDb::new(FileKernel::open(&store_path)?);
    let imported_commits = db.import_commit_batch(batch)?;
    let output = serde_json::json!({
        "path": store_path.display().to_string(),
        "input": input_path.display().to_string(),
        "imported_commits": imported_commits,
    });
    println!("{}", serde_json::to_string_pretty(&output)?);
}
```

- [ ] **Step 4: Run targeted CLI tests and verify GREEN**

Run:

```bash
cargo test -p continuitydb-cli commit
```

Expected: commit backup/restore CLI tests pass.

## Task 3: Docs and Full Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-cli-commit-backup-restore.md`

- [ ] **Step 1: Update README**

Add to Current Scope near the existing CLI bullet:

```markdown
- CLI commit backup and restore commands over versioned export envelopes.
```

- [ ] **Step 2: Update roadmap**

Add a CLI milestone after compaction:

```markdown
3. Expose commit backup and restore from the CLI. Implemented `continuitydb export-commits <store-path> <output-path>` and `continuitydb import-commits <store-path> <input-path>` over the versioned commit export envelope so file-backed stores can be copied through a validated portable backup file.
```

- [ ] **Step 3: Run full verification gate**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

Expected: every command exits 0.

- [ ] **Step 4: Commit**

Run:

```bash
git add README.md docs/roadmap.md crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs docs/superpowers/plans/2026-05-20-cli-commit-backup-restore.md
git commit -m "feat: add cli commit backup restore"
```
