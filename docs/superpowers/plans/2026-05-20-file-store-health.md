# File Store Health Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a file-backed store health report that classifies readable stores as canonical or compaction-worthy.

**Architecture:** `continuitydb-kernel` owns file-format health metadata collected by the existing validated JSONL parser. `FileKernel` stores the report alongside its rebuilt index, updates it after appends and compaction, and exposes it through file-specific API/CLI surfaces without changing the generic `StorageKernel` trait.

**Tech Stack:** Rust, existing `continuitydb-kernel`, `continuitydb-api`, and `continuitydb-cli` crates, serde_json, assert_cmd tests.

---

## File Structure

- Modify `crates/continuitydb-kernel/src/lib.rs`: add `FileKernelHealth`, collect health in `read_log_from_path`, store it in `FileKernel`, expose `FileKernel::health`, and add kernel tests.
- Modify `crates/continuitydb-api/src/lib.rs`: expose `file_store_health` and add API coverage.
- Modify `crates/continuitydb-cli/src/main.rs`: include `health` in `inspect-kernel` JSON.
- Modify `crates/continuitydb-cli/tests/cli.rs`: assert `health` output for new and legacy-readable stores.
- Modify `README.md`: add file-store health reporting to current scope.
- Modify `docs/roadmap.md`: add storage/API/CLI milestones.

## Task 1: Kernel Health Metadata

**Files:**
- Modify: `crates/continuitydb-kernel/src/lib.rs`

- [ ] **Step 1: Write failing kernel tests**

Add these tests near the existing file-kernel status and compaction tests:

```rust
#[test]
fn file_kernel_health_reports_new_store_as_canonical() -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_kernel_path("continuitydb-file-kernel-health-new");
    let kernel = FileKernel::open(&path)?;

    let health = kernel.health();

    assert!(health.has_header);
    assert_eq!(health.legacy_raw_cells, 0);
    assert_eq!(health.checksum_free_records, 0);
    assert_eq!(health.canonical_records, 0);
    assert!(!health.compaction_recommended);

    fs::remove_file(path)?;
    Ok(())
}

#[test]
fn file_kernel_health_recommends_compaction_for_legacy_raw_cells(
) -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_kernel_path("continuitydb-file-kernel-health-legacy");
    let committed_at = test_commit_time()?;
    let commit_id = CommitId::new();
    let mut cell = sample_cell("project:continuitydb:health-legacy", 0.91, 12)?;
    cell.system_time = continuitydb_core::SystemTimeRange::open_from(committed_at);
    cell.commit_id = commit_id;
    fs::write(&path, format!("{}\n", serde_json::to_string(&cell)?))?;

    let kernel = FileKernel::open(&path)?;
    let health = kernel.health();

    assert!(!health.has_header);
    assert_eq!(health.legacy_raw_cells, 1);
    assert_eq!(health.checksum_free_records, 0);
    assert_eq!(health.canonical_records, 0);
    assert!(health.compaction_recommended);

    fs::remove_file(path)?;
    Ok(())
}

#[test]
fn file_kernel_health_recommends_compaction_for_checksum_free_records(
) -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_kernel_path("continuitydb-file-kernel-health-checksum-free");
    let committed_at = test_commit_time()?;
    let commit_id = CommitId::new();
    let mut cell = sample_cell("project:continuitydb:health-checksum-free", 0.91, 12)?;
    cell.system_time = continuitydb_core::SystemTimeRange::open_from(committed_at);
    cell.commit_id = commit_id;
    let manifest = continuitydb_core::CommitManifest {
        commit_id,
        committed_at,
        cell_ids: vec![cell.id],
    };
    fs::write(
        &path,
        format!(
            "{}\n{}\n{}\n",
            serde_json::json!({
                "type": "header",
                "format": "continuitydb.file_kernel",
                "version": 1
            }),
            serde_json::json!({
                "type": "cell",
                "cell": cell
            }),
            serde_json::json!({
                "type": "commit",
                "manifest": manifest
            }),
        ),
    )?;

    let kernel = FileKernel::open(&path)?;
    let health = kernel.health();

    assert!(health.has_header);
    assert_eq!(health.legacy_raw_cells, 0);
    assert_eq!(health.checksum_free_records, 2);
    assert_eq!(health.canonical_records, 0);
    assert!(health.compaction_recommended);

    fs::remove_file(path)?;
    Ok(())
}

#[test]
fn file_kernel_compaction_updates_health_to_canonical(
) -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_kernel_path("continuitydb-file-kernel-health-compact");
    let committed_at = test_commit_time()?;
    let commit_id = CommitId::new();
    let mut cell = sample_cell("project:continuitydb:health-compact", 0.91, 12)?;
    cell.system_time = continuitydb_core::SystemTimeRange::open_from(committed_at);
    cell.commit_id = commit_id;
    fs::write(&path, format!("{}\n", serde_json::to_string(&cell)?))?;
    let mut kernel = FileKernel::open(&path)?;

    kernel.compact()?;
    let health = kernel.health();

    assert!(health.has_header);
    assert_eq!(health.legacy_raw_cells, 0);
    assert_eq!(health.checksum_free_records, 0);
    assert_eq!(health.canonical_records, 2);
    assert!(!health.compaction_recommended);

    fs::remove_file(path)?;
    Ok(())
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p continuitydb-kernel file_kernel_health --all-features
```

Expected: FAIL because `FileKernel::health` does not exist.

- [ ] **Step 3: Add health types and parser counters**

Add near `FileKernelStatus`:

```rust
/// Operational health report for a file-backed storage kernel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FileKernelHealth {
    /// Whether the log starts with the supported current header.
    pub has_header: bool,
    /// Number of legacy raw StateCell records accepted during open.
    pub legacy_raw_cells: usize,
    /// Number of typed cell or commit records that lacked checksums.
    pub checksum_free_records: usize,
    /// Number of typed cell or commit records with valid checksums.
    pub canonical_records: usize,
    /// Whether compaction should rewrite the store into the canonical format.
    pub compaction_recommended: bool,
}
```

Add an internal constructor:

```rust
impl FileKernelHealth {
    fn from_counts(
        has_header: bool,
        legacy_raw_cells: usize,
        checksum_free_records: usize,
        canonical_records: usize,
    ) -> Self {
        Self {
            has_header,
            legacy_raw_cells,
            checksum_free_records,
            canonical_records,
            compaction_recommended: !has_header
                || legacy_raw_cells > 0
                || checksum_free_records > 0,
        }
    }
}
```

Extend `FileKernelLog`:

```rust
health: FileKernelHealth,
```

Because `FileKernelLog` currently derives `Default`, also implement `Default` for `FileKernelHealth`:

```rust
impl Default for FileKernelHealth {
    fn default() -> Self {
        Self::from_counts(false, 0, 0, 0)
    }
}
```

In `read_log_from_path`, increment health counters:

```rust
log.health.has_header = true;
log.health.compaction_recommended = log.health.legacy_raw_cells > 0
    || log.health.checksum_free_records > 0;
```

For typed cell and commit records, call a helper before pushing:

```rust
record_file_health(&mut log.health, checksum.is_some());
```

For fallback raw StateCell records:

```rust
log.health.legacy_raw_cells += 1;
log.health.compaction_recommended = true;
```

Add helper:

```rust
fn record_file_health(health: &mut FileKernelHealth, has_checksum: bool) {
    if has_checksum {
        health.canonical_records += 1;
    } else {
        health.checksum_free_records += 1;
        health.compaction_recommended = true;
    }
}
```

- [ ] **Step 4: Store and expose health in FileKernel**

Update `FileKernel`:

```rust
pub struct FileKernel {
    path: PathBuf,
    index: FileKernelIndex,
    health: FileKernelHealth,
}
```

In `FileKernel::open`, preserve health before rebuilding the index:

```rust
let log = read_log_from_path(&path)?;
let health = log.health;
let index = FileKernelIndex::rebuild(log)?;

Ok(Self { path, index, health })
```

Add method:

```rust
/// Returns the file-format health report captured for this store.
pub fn health(&self) -> FileKernelHealth {
    self.health
}
```

In `compact`, preserve health from the compacted parse result:

```rust
let compacted_log = read_log_from_path(&temp_path)?;
let compacted_health = compacted_log.health;
let compacted_index = FileKernelIndex::rebuild(compacted_log)?;
...
self.index = compacted_index;
self.health = compacted_health;
```

In `append_cells_at_with_commit_id`, after durable append succeeds and before returning, increment canonical records for the newly written cell and commit records:

```rust
self.health.canonical_records += stamped.len() + usize::from(!stamped.is_empty());
```

- [ ] **Step 5: Run test to verify it passes**

Run:

```bash
cargo test -p continuitydb-kernel file_kernel_health --all-features
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/continuitydb-kernel/src/lib.rs
git commit -m "feat: add file kernel health report"
```

## Task 2: API and CLI Health Exposure

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`
- Modify: `crates/continuitydb-cli/src/main.rs`
- Modify: `crates/continuitydb-cli/tests/cli.rs`

- [ ] **Step 1: Write failing API test**

Add near the file-store status API test:

```rust
#[test]
fn api_reports_file_store_health() -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_file_kernel_path("api-file-store-health");
    let db = ContinuityDb::open_file(&path)?;

    let health = db.file_store_health();

    assert!(health.has_header);
    assert_eq!(health.legacy_raw_cells, 0);
    assert_eq!(health.checksum_free_records, 0);
    assert_eq!(health.canonical_records, 0);
    assert!(!health.compaction_recommended);

    fs::remove_file(path)?;
    Ok(())
}
```

- [ ] **Step 2: Update failing CLI tests**

In `cli_inspect_kernel_reports_file_capabilities`, add:

```rust
assert_eq!(json["health"]["has_header"].as_bool(), Some(true));
assert_eq!(json["health"]["legacy_raw_cells"].as_u64(), Some(0));
assert_eq!(json["health"]["checksum_free_records"].as_u64(), Some(0));
assert_eq!(json["health"]["canonical_records"].as_u64(), Some(0));
assert_eq!(
    json["health"]["compaction_recommended"].as_bool(),
    Some(false)
);
```

Add a new CLI test:

```rust
#[test]
fn cli_inspect_kernel_reports_legacy_health() -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_store_path("continuitydb-cli-inspect-legacy-health");
    write_legacy_store(&path)?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("inspect-kernel")
        .arg(&path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(json["health"]["has_header"].as_bool(), Some(false));
    assert_eq!(json["health"]["legacy_raw_cells"].as_u64(), Some(1));
    assert_eq!(
        json["health"]["compaction_recommended"].as_bool(),
        Some(true)
    );

    fs::remove_file(path)?;
    Ok(())
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run:

```bash
cargo test -p continuitydb-api file_store_health --all-features
cargo test -p continuitydb-cli inspect_kernel_reports_ --all-features
```

Expected: FAIL because `file_store_health` and CLI `health` output do not exist.

- [ ] **Step 4: Implement API health**

Add `FileKernelHealth` to the API import:

```rust
use continuitydb_kernel::{
    CellLookup, CommitManifestLookup, FileKernel, FileKernelHealth, FileKernelStatus,
    KernelCapabilities, KernelError, KernelRequirements, StorageKernel,
};
```

Add to `impl ContinuityDb<FileKernel>`:

```rust
/// Returns the file-format health report for the backing file store.
pub fn file_store_health(&self) -> FileKernelHealth {
    self.kernel.health()
}
```

- [ ] **Step 5: Implement CLI health JSON**

Add helper:

```rust
fn file_health_json(db: &ContinuityDb<continuitydb_kernel::FileKernel>) -> serde_json::Value {
    let health = db.file_store_health();
    serde_json::json!({
        "has_header": health.has_header,
        "legacy_raw_cells": health.legacy_raw_cells,
        "checksum_free_records": health.checksum_free_records,
        "canonical_records": health.canonical_records,
        "compaction_recommended": health.compaction_recommended,
    })
}
```

In `inspect-kernel`, add:

```rust
let health = file_health_json(&db);
```

and include it in output:

```rust
"health": health,
```

- [ ] **Step 6: Run tests to verify they pass**

Run:

```bash
cargo test -p continuitydb-api file_store_health --all-features
cargo test -p continuitydb-cli inspect_kernel_reports_ --all-features
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/continuitydb-api/src/lib.rs crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs
git commit -m "feat: expose file store health"
```

## Task 3: Docs and Full Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [ ] **Step 1: Update docs**

Add this README current-scope bullet near the file-store status item:

```markdown
- File-backed store health reporting for canonical and compaction-worthy logs.
```

Add these roadmap milestones:

Storage Kernel:

```markdown
26. Add file-backed store health reporting. Implemented `FileKernelHealth` and `FileKernel::health` so validated readable stores report whether they are canonical, legacy raw, checksum-free, or compaction-worthy.
```

Native API:

```markdown
15. Add native file-store health reporting. Implemented `ContinuityDb<FileKernel>::file_store_health` so embedders can inspect file format health without depending on kernel internals.
```

CLI:

```markdown
7. Include file-store health in kernel inspection. Extended `continuitydb inspect-kernel` JSON with file-format health and compaction recommendation metadata.
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
git commit -m "docs: record file store health"
```

