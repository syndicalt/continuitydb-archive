# CLI Incremental Commit Export Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add cursor and limit support to `continuitydb export-commits` so CLI backup/sync workflows can export commit pages incrementally.

**Architecture:** Add stable string parsing/display for `CommitId`, then wire optional `--after` and `--limit` CLI flags into the existing `CommitManifestLookup`-driven native export helper. Existing full-export behavior remains the default when flags are omitted.

**Tech Stack:** Rust, `continuitydb-core`, `continuitydb-cli`, clap, assert_cmd, serde_json, uuid.

---

## File Structure

- Modify `crates/continuitydb-core/src/cell.rs`: add `Display` and `FromStr` for `CommitId`.
- Modify `crates/continuitydb-core/src/lib.rs`: add `CommitId` string tests.
- Modify `crates/continuitydb-cli/src/main.rs`: add `--after` and `--limit` export flags and build `CommitManifestLookup` from them.
- Modify `crates/continuitydb-cli/tests/cli.rs`: add CLI incremental export tests.
- Modify `README.md`: add CLI incremental commit export to current scope.
- Modify `docs/roadmap.md`: add CLI milestone.

## Task 1: CommitId String Boundary

**Files:**
- Modify: `crates/continuitydb-core/src/cell.rs`
- Modify: `crates/continuitydb-core/src/lib.rs`

- [ ] **Step 1: Write failing core tests**

Add these tests near existing `CommitId` tests in `crates/continuitydb-core/src/lib.rs`:

```rust
#[test]
fn commit_id_displays_and_parses_uuid_text() -> Result<(), Box<dyn std::error::Error>> {
    let commit_id = CommitId::new();
    let text = commit_id.to_string();

    let parsed: CommitId = text.parse()?;

    assert_eq!(parsed, commit_id);
    assert_eq!(text.len(), 36);
    Ok(())
}

#[test]
fn commit_id_rejects_invalid_uuid_text() {
    let result = "not-a-uuid".parse::<CommitId>();

    assert!(result.is_err());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p continuitydb-core commit_id_ --all-features
```

Expected: FAIL because `CommitId` does not implement `Display` or `FromStr`.

- [ ] **Step 3: Implement CommitId string traits**

In `crates/continuitydb-core/src/cell.rs`, add imports near the top:

```rust
use std::{fmt, str::FromStr};
```

Add below `impl Default for CommitId`:

```rust
impl fmt::Display for CommitId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for CommitId {
    type Err = uuid::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(value).map(Self)
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run:

```bash
cargo test -p continuitydb-core commit_id_ --all-features
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/continuitydb-core/src/cell.rs crates/continuitydb-core/src/lib.rs
git commit -m "feat: add commit id string parsing"
```

## Task 2: CLI Export Cursor and Limit

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`
- Modify: `crates/continuitydb-cli/tests/cli.rs`

- [ ] **Step 1: Write failing CLI tests**

Add this helper near `write_committed_store` in `crates/continuitydb-cli/tests/cli.rs`:

```rust
fn write_two_commit_store(path: &PathBuf) -> Result<Vec<CommitId>, Box<dyn std::error::Error>> {
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
    let mut db = ContinuityDb::new(FileKernel::open(path)?);
    db.ingest_cells_at_with_commit_id(
        vec![test_cell("project:continuitydb:cli-export-page-first")?],
        first_time,
        first_commit,
    )?;
    db.ingest_cells_at_with_commit_id(
        vec![test_cell("project:continuitydb:cli-export-page-second")?],
        second_time,
        second_commit,
    )?;
    Ok(vec![first_commit, second_commit])
}
```

Add these tests near existing export/import CLI tests:

```rust
#[test]
fn cli_export_commits_limit_outputs_first_page_cursor() -> Result<(), Box<dyn std::error::Error>> {
    let source_path = temp_store_path("continuitydb-cli-export-limit-source");
    let backup_path = temp_store_path("continuitydb-cli-export-limit-backup");
    let commits = write_two_commit_store(&source_path)?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("export-commits")
        .arg(&source_path)
        .arg(&backup_path)
        .arg("--limit")
        .arg("1")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;
    let batch = ContinuityDb::<FileKernel>::decode_commit_export_json(&fs::read(&backup_path)?)?;

    assert_eq!(json["exported_commits"].as_u64(), Some(1));
    assert_eq!(json["next_after"].as_str(), Some(commits[0].to_string().as_str()));
    assert_eq!(batch.slices.len(), 1);
    assert_eq!(batch.slices[0].manifest.commit_id, commits[0]);

    fs::remove_file(source_path)?;
    fs::remove_file(backup_path)?;
    Ok(())
}

#[test]
fn cli_export_commits_after_cursor_outputs_next_page() -> Result<(), Box<dyn std::error::Error>> {
    let source_path = temp_store_path("continuitydb-cli-export-after-source");
    let backup_path = temp_store_path("continuitydb-cli-export-after-backup");
    let commits = write_two_commit_store(&source_path)?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("export-commits")
        .arg(&source_path)
        .arg(&backup_path)
        .arg("--after")
        .arg(commits[0].to_string())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;
    let batch = ContinuityDb::<FileKernel>::decode_commit_export_json(&fs::read(&backup_path)?)?;

    assert_eq!(json["exported_commits"].as_u64(), Some(1));
    assert_eq!(json["next_after"].as_str(), Some(commits[1].to_string().as_str()));
    assert_eq!(batch.slices.len(), 1);
    assert_eq!(batch.slices[0].manifest.commit_id, commits[1]);

    fs::remove_file(source_path)?;
    fs::remove_file(backup_path)?;
    Ok(())
}

#[test]
fn cli_export_commits_fails_for_unknown_after_cursor() -> Result<(), Box<dyn std::error::Error>> {
    let source_path = temp_store_path("continuitydb-cli-export-after-unknown-source");
    let backup_path = temp_store_path("continuitydb-cli-export-after-unknown-backup");
    write_committed_store(&source_path, "project:continuitydb:cli-export-after-unknown")?;

    Command::cargo_bin("continuitydb")?
        .arg("export-commits")
        .arg(&source_path)
        .arg(&backup_path)
        .arg("--after")
        .arg(CommitId::new().to_string())
        .assert()
        .failure();

    fs::remove_file(source_path)?;
    let _ = fs::remove_file(backup_path);
    Ok(())
}

#[test]
fn cli_export_commits_rejects_invalid_after_cursor() -> Result<(), Box<dyn std::error::Error>> {
    let source_path = temp_store_path("continuitydb-cli-export-after-invalid-source");
    let backup_path = temp_store_path("continuitydb-cli-export-after-invalid-backup");
    write_committed_store(&source_path, "project:continuitydb:cli-export-after-invalid")?;

    Command::cargo_bin("continuitydb")?
        .arg("export-commits")
        .arg(&source_path)
        .arg(&backup_path)
        .arg("--after")
        .arg("not-a-uuid")
        .assert()
        .failure();

    fs::remove_file(source_path)?;
    let _ = fs::remove_file(backup_path);
    Ok(())
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p continuitydb-cli export_commits_ --all-features
```

Expected: FAIL because `export-commits` does not accept `--after` or `--limit`.

- [ ] **Step 3: Implement CLI flags**

Update imports in `crates/continuitydb-cli/src/main.rs`:

```rust
use continuitydb_core::{
    ActivationState, Answerability, CellCost, CellPayload, Citation, CommitId, Confidence,
    Evidence, Scope, SemanticAnchor, SourceId, StateCell, StateCellId, TrustSignal,
    ValidTimeRange,
};
```

Update `ExportCommits`:

```rust
ExportCommits {
    /// Path to the JSONL file-backed store.
    store_path: PathBuf,
    /// Path to write the versioned JSON commit export envelope.
    output_path: PathBuf,
    /// Exclusive commit cursor to start after.
    #[arg(long = "after")]
    after: Option<CommitId>,
    /// Maximum number of commits to export.
    #[arg(long = "limit")]
    limit: Option<usize>,
},
```

Update the export match arm:

```rust
Some(Command::ExportCommits {
    store_path,
    output_path,
    after,
    limit,
}) => {
    let db = open_file_database(&store_path)?;
    let summary = db.export_commits_json_file(CommitManifestLookup { after, limit }, &output_path)?;
    let output = serde_json::json!({
        "path": store_path.display().to_string(),
        "output": output_path.display().to_string(),
        "exported_commits": summary.exported_commits,
        "next_after": summary.next_after,
    });
    println!("{}", serde_json::to_string_pretty(&output)?);
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run:

```bash
cargo test -p continuitydb-cli export_commits_ --all-features
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs
git commit -m "feat: add cli incremental commit export"
```

## Task 3: Docs and Full Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [ ] **Step 1: Update docs**

Add this README current-scope bullet near CLI commit backup and restore:

```markdown
- CLI incremental commit export for cursor-based backup and sync workflows.
```

Add this CLI roadmap milestone after dry-run import validation:

```markdown
11. Add incremental commit export. Implemented `continuitydb export-commits --after --limit` so operators can page commit backups through the same cursor semantics exposed by the native API.
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
git commit -m "docs: record cli incremental commit export"
```
