# Conditional File Compaction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add explicit conditional compaction so file-backed stores are rewritten only when health recommends compaction.

**Architecture:** `continuitydb-api` owns the embeddable conditional maintenance helper and returns a typed before/after summary. The CLI `compact-file --if-needed` delegates to that helper and prints the summary, while the existing unconditional compaction path remains unchanged.

**Tech Stack:** Rust, existing `continuitydb-api` and `continuitydb-cli` crates, serde serialization, clap, assert_cmd.

---

## File Structure

- Modify `crates/continuitydb-api/src/lib.rs`: add `FileCompactionSummary`, `compact_file_store_if_needed`, and API tests.
- Modify `crates/continuitydb-cli/src/main.rs`: add `compact-file --if-needed`, summary JSON helpers, and conditional routing.
- Modify `crates/continuitydb-cli/tests/cli.rs`: add CLI conditional compaction tests.
- Modify `README.md`: add conditional file compaction to current scope.
- Modify `docs/roadmap.md`: add Native API and CLI milestones.

## Task 1: Native API Conditional Compaction

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`

- [ ] **Step 1: Write failing API tests**

Add these tests near existing file compaction tests:

```rust
#[test]
fn api_skips_file_compaction_when_store_is_canonical(
) -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_file_kernel_path("api-file-compact-if-needed-canonical");
    let mut db = ContinuityDb::open_file(&path)?;

    let summary = db.compact_file_store_if_needed()?;

    assert!(!summary.compacted);
    assert_eq!(summary.before, summary.after);
    assert!(!summary.after.compaction_recommended);

    fs::remove_file(path)?;
    Ok(())
}

#[test]
fn api_compacts_file_store_when_health_recommends_it(
) -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_file_kernel_path("api-file-compact-if-needed-legacy");
    write_legacy_file_store(&path)?;
    let mut db = ContinuityDb::open_file(&path)?;

    let summary = db.compact_file_store_if_needed()?;

    assert!(summary.compacted);
    assert!(summary.before.compaction_recommended);
    assert!(!summary.after.compaction_recommended);
    assert_eq!(summary.after.legacy_raw_cells, 0);
    assert!(summary.after.has_header);

    fs::remove_file(path)?;
    Ok(())
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p continuitydb-api compact_if_needed --all-features
```

Expected: FAIL because `compact_file_store_if_needed` and `FileCompactionSummary` do not exist.

- [ ] **Step 3: Implement API summary and helper**

Add near `CommitExportFileSummary`:

```rust
/// Summary of a conditional file-store compaction attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FileCompactionSummary {
    /// Whether the backing store was rewritten.
    pub compacted: bool,
    /// File-store health before the maintenance decision.
    pub before: FileKernelHealth,
    /// File-store health after the maintenance decision.
    pub after: FileKernelHealth,
}
```

Add to `impl ContinuityDb<FileKernel>`:

```rust
/// Rewrites a file-backed store only when health recommends compaction.
pub fn compact_file_store_if_needed(&mut self) -> Result<FileCompactionSummary, ContinuityError> {
    let before = self.file_store_health();
    if !before.compaction_recommended {
        return Ok(FileCompactionSummary {
            compacted: false,
            before,
            after: before,
        });
    }

    self.compact_file_store()?;
    let after = self.file_store_health();
    Ok(FileCompactionSummary {
        compacted: true,
        before,
        after,
    })
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run:

```bash
cargo test -p continuitydb-api compact_if_needed --all-features
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/continuitydb-api/src/lib.rs
git commit -m "feat: add conditional file compaction api"
```

## Task 2: CLI Conditional Compaction

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`
- Modify: `crates/continuitydb-cli/tests/cli.rs`

- [ ] **Step 1: Write failing CLI tests**

Add these tests near existing compact-file tests:

```rust
#[test]
fn cli_compact_file_if_needed_skips_canonical_store(
) -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_store_path("continuitydb-cli-compact-if-needed-canonical");

    let output = Command::cargo_bin("continuitydb")?
        .arg("compact-file")
        .arg(&path)
        .arg("--if-needed")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(json["compacted"].as_bool(), Some(false));
    assert_eq!(
        json["after"]["compaction_recommended"].as_bool(),
        Some(false)
    );

    fs::remove_file(path)?;
    Ok(())
}

#[test]
fn cli_compact_file_if_needed_compacts_legacy_store(
) -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_store_path("continuitydb-cli-compact-if-needed-legacy");
    write_legacy_store(&path)?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("compact-file")
        .arg(&path)
        .arg("--if-needed")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(json["compacted"].as_bool(), Some(true));
    assert_eq!(
        json["before"]["compaction_recommended"].as_bool(),
        Some(true)
    );
    assert_eq!(
        json["after"]["compaction_recommended"].as_bool(),
        Some(false)
    );

    fs::remove_file(path)?;
    Ok(())
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p continuitydb-cli compact_file_if_needed --all-features
```

Expected: FAIL because `compact-file --if-needed` is not accepted.

- [ ] **Step 3: Implement CLI flag and JSON helpers**

Update `CompactFile`:

```rust
CompactFile {
    /// Path to the JSONL file-backed store.
    path: PathBuf,
    /// Skip rewriting when the store is already canonical.
    #[arg(long = "if-needed")]
    if_needed: bool,
},
```

Update match arm:

```rust
Some(Command::CompactFile { path, if_needed }) => {
    let mut db = open_file_database(&path)?;
    let output = if if_needed {
        let summary = db.compact_file_store_if_needed()?;
        serde_json::json!({
            "path": path.display().to_string(),
            "compacted": summary.compacted,
            "before": file_health_value(summary.before),
            "after": file_health_value(summary.after),
        })
    } else {
        db.compact_file_store()?;
        serde_json::json!({
            "path": path.display().to_string(),
            "compacted": true,
        })
    };
    println!("{}", serde_json::to_string_pretty(&output)?);
}
```

Add helper:

```rust
fn file_health_value(health: continuitydb_kernel::FileKernelHealth) -> serde_json::Value {
    serde_json::json!({
        "has_header": health.has_header,
        "legacy_raw_cells": health.legacy_raw_cells,
        "checksum_free_records": health.checksum_free_records,
        "canonical_records": health.canonical_records,
        "compaction_recommended": health.compaction_recommended,
    })
}
```

Refactor `file_health_json` to call `file_health_value(db.file_store_health())`.

- [ ] **Step 4: Run tests to verify they pass**

Run:

```bash
cargo test -p continuitydb-cli compact_file_if_needed --all-features
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs
git commit -m "feat: add cli conditional file compaction"
```

## Task 3: Docs and Full Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [ ] **Step 1: Update docs**

Add this README current-scope bullet near compaction:

```markdown
- Conditional file-store compaction for explicit maintenance automation.
```

Add these roadmap milestones:

Native API:

```markdown
17. Add conditional file-store compaction. Implemented `compact_file_store_if_needed` with before/after health summaries so embedders can automate explicit maintenance without rewriting canonical stores.
```

CLI:

```markdown
9. Add conditional file-store compaction. Implemented `continuitydb compact-file --if-needed` so operators can compact only when health recommends it.
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
git commit -m "docs: record conditional file compaction"
```

