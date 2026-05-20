# CLI Direct Commit Copy Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `continuitydb copy-commits` so operators can copy cursor-selected commit pages directly between two local file-backed stores without an intermediate JSON backup file.

**Architecture:** Reuse the native `ContinuityDb::copy_commits_from` API. The CLI opens a source file-backed database and a mutable target file-backed database, builds `CommitManifestLookup` from optional cursor flags, calls direct copy, and prints a JSON summary.

**Tech Stack:** Rust, `continuitydb-cli`, `continuitydb-api`, clap, assert_cmd, serde_json.

---

## File Structure

- Modify `crates/continuitydb-cli/src/main.rs`: add the `copy-commits` command and JSON output.
- Modify `crates/continuitydb-cli/tests/cli.rs`: add direct copy CLI tests.
- Modify `README.md`: add CLI direct commit copy to current scope.
- Modify `docs/roadmap.md`: add CLI direct commit copy milestone.

## Task 1: CLI Direct Copy Command

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`
- Modify: `crates/continuitydb-cli/tests/cli.rs`

- [ ] **Step 1: Write failing CLI tests**

Add these tests near existing commit backup/copy tests in `crates/continuitydb-cli/tests/cli.rs`:

```rust
#[test]
fn cli_copy_commits_limit_copies_first_page() -> Result<(), Box<dyn std::error::Error>> {
    let source_path = temp_store_path("continuitydb-cli-copy-limit-source");
    let target_path = temp_store_path("continuitydb-cli-copy-limit-target");
    let commits = write_two_commit_store(&source_path)?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("copy-commits")
        .arg(&source_path)
        .arg(&target_path)
        .arg("--limit")
        .arg("1")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;
    let target_batch = ContinuityDb::new(FileKernel::open(&target_path)?)
        .export_commits(CommitManifestLookup::default())?;

    assert_eq!(json["source"].as_str(), source_path.to_str());
    assert_eq!(json["target"].as_str(), target_path.to_str());
    assert_eq!(json["copied_commits"].as_u64(), Some(1));
    assert_eq!(json["next_after"].as_str(), Some(commits[0].to_string().as_str()));
    assert_eq!(target_batch.slices.len(), 1);
    assert_eq!(target_batch.slices[0].manifest.commit_id, commits[0]);

    fs::remove_file(source_path)?;
    fs::remove_file(target_path)?;
    Ok(())
}

#[test]
fn cli_copy_commits_after_cursor_copies_next_page() -> Result<(), Box<dyn std::error::Error>> {
    let source_path = temp_store_path("continuitydb-cli-copy-after-source");
    let target_path = temp_store_path("continuitydb-cli-copy-after-target");
    let commits = write_two_commit_store(&source_path)?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("copy-commits")
        .arg(&source_path)
        .arg(&target_path)
        .arg("--after")
        .arg(commits[0].to_string())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;
    let target_batch = ContinuityDb::new(FileKernel::open(&target_path)?)
        .export_commits(CommitManifestLookup::default())?;

    assert_eq!(json["copied_commits"].as_u64(), Some(1));
    assert_eq!(json["next_after"].as_str(), Some(commits[1].to_string().as_str()));
    assert_eq!(target_batch.slices.len(), 1);
    assert_eq!(target_batch.slices[0].manifest.commit_id, commits[1]);

    fs::remove_file(source_path)?;
    fs::remove_file(target_path)?;
    Ok(())
}

#[test]
fn cli_copy_commits_empty_source_reports_zero() -> Result<(), Box<dyn std::error::Error>> {
    let source_path = temp_store_path("continuitydb-cli-copy-empty-source");
    let target_path = temp_store_path("continuitydb-cli-copy-empty-target");

    let output = Command::cargo_bin("continuitydb")?
        .arg("copy-commits")
        .arg(&source_path)
        .arg(&target_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;
    let target_batch = ContinuityDb::new(FileKernel::open(&target_path)?)
        .export_commits(CommitManifestLookup::default())?;

    assert_eq!(json["copied_commits"].as_u64(), Some(0));
    assert!(json["next_after"].is_null());
    assert!(target_batch.slices.is_empty());

    fs::remove_file(source_path)?;
    fs::remove_file(target_path)?;
    Ok(())
}

#[test]
fn cli_copy_commits_rejects_duplicate_target_commit() -> Result<(), Box<dyn std::error::Error>> {
    let source_path = temp_store_path("continuitydb-cli-copy-duplicate-source");
    let target_path = temp_store_path("continuitydb-cli-copy-duplicate-target");
    let committed_at = Utc
        .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let commit_id = CommitId::new();
    let mut source = ContinuityDb::new(FileKernel::open(&source_path)?);
    source.ingest_cells_at_with_commit_id(
        vec![test_cell("project:continuitydb:cli-copy-duplicate-source")?],
        committed_at,
        commit_id,
    )?;
    let mut target = ContinuityDb::new(FileKernel::open(&target_path)?);
    target.ingest_cells_at_with_commit_id(
        vec![test_cell("project:continuitydb:cli-copy-duplicate-target")?],
        committed_at,
        commit_id,
    )?;

    Command::cargo_bin("continuitydb")?
        .arg("copy-commits")
        .arg(&source_path)
        .arg(&target_path)
        .assert()
        .failure();
    let target_batch = ContinuityDb::new(FileKernel::open(&target_path)?)
        .export_commits(CommitManifestLookup::default())?;

    assert_eq!(target_batch.slices.len(), 1);

    fs::remove_file(source_path)?;
    fs::remove_file(target_path)?;
    Ok(())
}

#[test]
fn cli_copy_commits_rejects_invalid_after_cursor() -> Result<(), Box<dyn std::error::Error>> {
    let source_path = temp_store_path("continuitydb-cli-copy-invalid-source");
    let target_path = temp_store_path("continuitydb-cli-copy-invalid-target");

    Command::cargo_bin("continuitydb")?
        .arg("copy-commits")
        .arg(&source_path)
        .arg(&target_path)
        .arg("--after")
        .arg("not-a-uuid")
        .assert()
        .failure();

    let _ = fs::remove_file(source_path);
    let _ = fs::remove_file(target_path);
    Ok(())
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p continuitydb-cli copy_commits --all-features
```

Expected: FAIL because `copy-commits` is not recognized.

- [ ] **Step 3: Implement CLI command**

Add a `CopyCommits` command variant in `crates/continuitydb-cli/src/main.rs` after `ExportCommits`:

```rust
/// Copy file-backed commit slices directly between two stores.
CopyCommits {
    /// Path to the source JSONL file-backed store.
    source_path: PathBuf,
    /// Path to the target JSONL file-backed store.
    target_path: PathBuf,
    /// Exclusive commit cursor to start after.
    #[arg(long = "after")]
    after: Option<CommitId>,
    /// Maximum number of commits to copy.
    #[arg(long = "limit")]
    limit: Option<usize>,
},
```

Add a match arm near `ExportCommits`:

```rust
Some(Command::CopyCommits {
    source_path,
    target_path,
    after,
    limit,
}) => {
    let source = open_file_database(&source_path)?;
    let mut target = open_file_database(&target_path)?;
    let summary = target.copy_commits_from(&source, CommitManifestLookup { after, limit })?;
    let output = serde_json::json!({
        "source": source_path.display().to_string(),
        "target": target_path.display().to_string(),
        "copied_commits": summary.imported_commits,
        "next_after": summary.next_after,
    });
    println!("{}", serde_json::to_string_pretty(&output)?);
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run:

```bash
cargo test -p continuitydb-cli copy_commits --all-features
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs
git commit -m "feat: add cli direct commit copy"
```

## Task 2: Docs and Full Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [ ] **Step 1: Update docs**

Add this README current-scope bullet near CLI backup/sync bullets:

```markdown
- CLI direct commit copy for local file-backed sync.
```

Add this CLI milestone after import cursor reporting:

```markdown
13. Add direct commit copy. Implemented `continuitydb copy-commits` so operators can copy cursor-selected commit pages between local file-backed stores without writing an intermediate backup file.
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
git commit -m "docs: record cli direct commit copy"
```
