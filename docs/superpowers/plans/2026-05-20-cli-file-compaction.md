# CLI File Compaction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `continuitydb compact-file <path>` to compact a JSONL file-backed store from the command line.

**Architecture:** Keep the CLI thin. The command opens `FileKernel`, wraps it in `ContinuityDb<FileKernel>`, calls `compact_file_store`, and prints deterministic JSON.

**Tech Stack:** Rust, clap, assert_cmd, serde_json, existing `continuitydb-api` and `continuitydb-kernel`.

---

## File Structure

- Modify `crates/continuitydb-cli/Cargo.toml`: add dependency on `continuitydb-api`.
- Modify `crates/continuitydb-cli/src/main.rs`: add `compact-file` command and output type.
- Modify `crates/continuitydb-cli/tests/cli.rs`: add CLI tests.
- Modify `README.md`: add CLI compaction command to current scope.
- Modify `docs/roadmap.md`: add CLI milestone.

## Task 1: Failing CLI Tests

**Files:**
- Modify: `crates/continuitydb-cli/tests/cli.rs`

- [ ] **Step 1: Add imports and helper functions**

Add imports:

```rust
use chrono::{TimeZone, Utc};
use continuitydb_core::{
    Answerability, CellCost, CellPayload, Citation, CommitId, Confidence, Evidence, Scope,
    SemanticAnchor, SourceId, StateCell, StateCellId, TrustSignal, ValidTimeRange,
};
use std::{fs, path::PathBuf};
```

Add helpers:

```rust
fn temp_store_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("{name}-{:?}.jsonl", StateCellId::new()))
}

fn test_cell(anchor: &str) -> Result<StateCell, Box<dyn std::error::Error>> {
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
                locator: "test://cli".to_string(),
            },
            confidence: Confidence::new(0.91)?,
            trust: vec![TrustSignal::DirectObservation],
        }],
        CellPayload::Text(anchor.to_string()),
        CellCost::new(12, 0)?,
    )
    .map_err(Into::into)
}

fn write_legacy_store(path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let committed_at = Utc
        .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let commit_id = CommitId::new();
    let mut cell = test_cell("project:continuitydb:cli-compact")?;
    cell.system_time = continuitydb_core::SystemTimeRange::open_from(committed_at);
    cell.commit_id = commit_id;
    fs::write(path, format!("{}\n", serde_json::to_string(&cell)?))?;
    Ok(())
}
```

- [ ] **Step 2: Add compact-file tests**

Add:

```rust
#[test]
fn cli_compact_file_rewrites_legacy_store() -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_store_path("continuitydb-cli-compact");
    write_legacy_store(&path)?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("compact-file")
        .arg(&path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;
    let records = fs::read_to_string(&path)?
        .lines()
        .map(str::to_string)
        .collect::<Vec<_>>();

    assert_eq!(json["compacted"].as_bool(), Some(true));
    assert_eq!(json["path"].as_str(), path.to_str());
    assert_eq!(records.len(), 3);
    assert!(records[0].contains(r#""type":"header""#));
    assert!(records[1].contains(r#""type":"cell""#));
    assert!(records[1].contains(r#""checksum":"continuitydb-fnv1a64:"#));
    assert!(records[2].contains(r#""type":"commit""#));
    assert!(records[2].contains(r#""checksum":"continuitydb-fnv1a64:"#));
    fs::remove_file(path)?;
    Ok(())
}

#[test]
fn cli_compact_file_fails_for_corrupt_store() -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_store_path("continuitydb-cli-compact-corrupt");
    fs::write(&path, "{not valid json}\n")?;

    Command::cargo_bin("continuitydb")?
        .arg("compact-file")
        .arg(&path)
        .assert()
        .failure();

    fs::remove_file(path)?;
    Ok(())
}
```

- [ ] **Step 3: Run targeted tests and verify RED**

Run:

```bash
cargo test -p continuitydb-cli compact_file
```

Expected: tests fail because `compact-file` is not a recognized command.

## Task 2: CLI Implementation

**Files:**
- Modify: `crates/continuitydb-cli/Cargo.toml`
- Modify: `crates/continuitydb-cli/src/main.rs`

- [ ] **Step 1: Add API dependency**

Add:

```toml
continuitydb-api = { path = "../continuitydb-api" }
```

- [ ] **Step 2: Add imports**

Add to `src/main.rs`:

```rust
use continuitydb_api::ContinuityDb;
use continuitydb_kernel::{FileKernel, StorageKernel};
use serde::Serialize;
use std::path::PathBuf;
```

Adjust the existing kernel import so `StorageKernel` is not duplicated.

- [ ] **Step 3: Add command and output type**

Extend `Command`:

```rust
    /// Compact a JSONL file-backed store into the canonical durable record format.
    CompactFile {
        /// Path to the JSONL file-backed store.
        path: PathBuf,
    },
```

Add:

```rust
#[derive(Debug, Serialize)]
struct CompactFileOutput {
    path: String,
    compacted: bool,
}
```

- [ ] **Step 4: Handle the command**

Add match arm:

```rust
        Some(Command::CompactFile { path }) => {
            let mut db = ContinuityDb::new(FileKernel::open(&path)?);
            db.compact_file_store()?;
            let output = CompactFileOutput {
                path: path.display().to_string(),
                compacted: true,
            };
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
```

- [ ] **Step 5: Run targeted tests and verify GREEN**

Run:

```bash
cargo test -p continuitydb-cli compact_file
```

Expected: compact-file CLI tests pass.

## Task 3: Docs and Full Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [ ] **Step 1: Update README current scope**

Add this bullet after `A thin CLI over library APIs.`:

```markdown
- CLI file-store compaction command.
```

- [ ] **Step 2: Update roadmap CLI milestones**

Add this milestone after demo checkout:

```markdown
2. Expose JSONL file-store compaction from the CLI. Implemented `continuitydb compact-file <path>` so operators can rewrite file-backed stores into the canonical durable record format and receive deterministic JSON status output.
```

- [ ] **Step 3: Run full verification**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

Expected: all commands exit 0.

- [ ] **Step 4: Commit**

Run:

```bash
git add crates/continuitydb-cli/Cargo.toml crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs README.md docs/roadmap.md Cargo.lock
git commit -m "feat: add cli file compaction"
```
