# File Store Status Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add operational status metadata for file-backed stores and expose it through the native API and `inspect-kernel` CLI output.

**Architecture:** `continuitydb-kernel` owns `FileKernelStatus` and computes counts from the validated in-memory index plus file size from filesystem metadata. `continuitydb-api` forwards that status for `ContinuityDb<FileKernel>`. The CLI serializes the status into existing inspection JSON.

**Tech Stack:** Rust, existing `continuitydb-kernel`, `continuitydb-api`, and `continuitydb-cli` crates, serde_json, assert_cmd tests.

---

## File Structure

- Modify `crates/continuitydb-kernel/src/lib.rs`: add `FileKernelStatus`, `FileKernel::status`, and kernel tests.
- Modify `crates/continuitydb-api/src/lib.rs`: expose `file_store_status` and API test.
- Modify `crates/continuitydb-cli/src/main.rs`: include status JSON in `inspect-kernel`.
- Modify `crates/continuitydb-cli/tests/cli.rs`: assert status JSON exists.
- Modify `README.md`: add file-store status to current scope.
- Modify `docs/roadmap.md`: add storage/API/CLI milestones.

## Task 1: Kernel File Store Status

**Files:**
- Modify: `crates/continuitydb-kernel/src/lib.rs`

- [ ] **Step 1: Write failing kernel tests**

Add these tests near the existing file-kernel tests:

```rust
#[test]
fn file_kernel_status_reports_empty_store() -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_kernel_path("continuitydb-file-kernel-status-empty");
    let kernel = FileKernel::open(&path)?;

    let status = kernel.status()?;

    assert_eq!(status.cell_count, 0);
    assert_eq!(status.commit_count, 0);
    assert!(status.file_size_bytes > 0);

    fs::remove_file(path)?;
    Ok(())
}

#[test]
fn file_kernel_status_reports_visible_cells_and_commits(
) -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_kernel_path("continuitydb-file-kernel-status-populated");
    let committed_at = test_commit_time()?;
    let commit_id = CommitId::new();
    let first = sample_cell("project:continuitydb:status-first", 0.91, 12)?;
    let second = sample_cell("project:continuitydb:status-second", 0.83, 15)?;
    let mut kernel = FileKernel::open(&path)?;
    kernel.append_cells_at_with_commit_id(vec![first, second], committed_at, commit_id)?;

    let status = kernel.status()?;

    assert_eq!(status.cell_count, 2);
    assert_eq!(status.commit_count, 1);
    assert!(status.file_size_bytes > 0);

    fs::remove_file(path)?;
    Ok(())
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p continuitydb-kernel file_kernel_status --all-features
```

Expected: FAIL because `FileKernel::status` and `FileKernelStatus` do not exist.

- [ ] **Step 3: Implement kernel status**

Add near `FileKernel`:

```rust
/// Observable status for a file-backed storage kernel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FileKernelStatus {
    /// Number of visible StateCells.
    pub cell_count: usize,
    /// Number of visible commit manifests.
    pub commit_count: usize,
    /// Current durable file size in bytes.
    pub file_size_bytes: u64,
}
```

Add to `impl FileKernel`:

```rust
/// Returns observable status for the backing file store.
pub fn status(&self) -> Result<FileKernelStatus, KernelError> {
    let file_size_bytes = fs::metadata(&self.path)
        .map_err(|_error| KernelError::StoreIo)?
        .len();
    Ok(FileKernelStatus {
        cell_count: self.index.cells.len(),
        commit_count: self.index.manifest_order.len(),
        file_size_bytes,
    })
}
```

- [ ] **Step 4: Run test to verify it passes**

Run:

```bash
cargo test -p continuitydb-kernel file_kernel_status --all-features
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/continuitydb-kernel/src/lib.rs
git commit -m "feat: add file kernel status"
```

## Task 2: API and CLI Status Exposure

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`
- Modify: `crates/continuitydb-cli/src/main.rs`
- Modify: `crates/continuitydb-cli/tests/cli.rs`

- [ ] **Step 1: Write failing API test**

In `crates/continuitydb-api/src/lib.rs`, add near the file-open tests:

```rust
#[test]
fn api_reports_file_store_status() -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_file_kernel_path("api-file-store-status");
    let mut db = ContinuityDb::open_file(&path)?;
    db.ingest_cells_at_with_commit_id(
        vec![sample_cell("project:continuitydb:file-status", 0.91, 12)?],
        Utc.with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?,
        CommitId::new(),
    )?;

    let status = db.file_store_status()?;

    assert_eq!(status.cell_count, 1);
    assert_eq!(status.commit_count, 1);
    assert!(status.file_size_bytes > 0);

    fs::remove_file(path)?;
    Ok(())
}
```

- [ ] **Step 2: Update failing CLI assertion**

In `cli_inspect_kernel_reports_file_capabilities`, add:

```rust
assert_eq!(json["status"]["cell_count"].as_u64(), Some(0));
assert_eq!(json["status"]["commit_count"].as_u64(), Some(0));
assert!(json["status"]["file_size_bytes"].as_u64().unwrap_or_default() > 0);
```

- [ ] **Step 3: Run tests to verify they fail**

Run:

```bash
cargo test -p continuitydb-api file_store_status --all-features
cargo test -p continuitydb-cli inspect_kernel_reports_file_capabilities --all-features
```

Expected: FAIL because `file_store_status` and CLI `status` output do not exist.

- [ ] **Step 4: Implement API status**

Add `FileKernelStatus` to the API import:

```rust
use continuitydb_kernel::{
    CellLookup, CommitManifestLookup, FileKernel, FileKernelStatus, KernelCapabilities,
    KernelError, KernelRequirements, StorageKernel,
};
```

Add to `impl ContinuityDb<FileKernel>`:

```rust
/// Returns observable status for the backing file store.
pub fn file_store_status(&self) -> Result<FileKernelStatus, ContinuityError> {
    self.kernel.status().map_err(Into::into)
}
```

- [ ] **Step 5: Implement CLI status JSON**

Update the top import:

```rust
use continuitydb_api::{ContinuityDb, ContinuityError};
```

Add helper:

```rust
fn file_status_json(
    db: &ContinuityDb<continuitydb_kernel::FileKernel>,
) -> Result<serde_json::Value, ContinuityError> {
    let status = db.file_store_status()?;
    Ok(serde_json::json!({
        "cell_count": status.cell_count,
        "commit_count": status.commit_count,
        "file_size_bytes": status.file_size_bytes,
    }))
}
```

In `inspect-kernel`, add:

```rust
let status = file_status_json(&db)?;
```

and include it in output:

```rust
"status": status,
```

- [ ] **Step 6: Run tests to verify they pass**

Run:

```bash
cargo test -p continuitydb-api file_store_status --all-features
cargo test -p continuitydb-cli inspect_kernel_reports_file_capabilities --all-features
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/continuitydb-api/src/lib.rs crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs
git commit -m "feat: expose file store status"
```

## Task 3: Docs and Full Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [ ] **Step 1: Update docs**

Add this README current-scope bullet near the file-kernel capability/status items:

```markdown
- File-backed store status for visible cell/commit counts and durable file size.
```

Add these roadmap milestones:

Storage Kernel:

```markdown
25. Add file-backed store status. Implemented `FileKernelStatus` and `FileKernel::status` so operators and embedders can inspect visible cell counts, commit counts, and durable file size after open-time validation.
```

Native API:

```markdown
14. Add native file-store status. Implemented `ContinuityDb<FileKernel>::file_store_status` so embedders can inspect file-backed store shape without depending on kernel internals.
```

CLI:

```markdown
6. Include file-store status in kernel inspection. Extended `continuitydb inspect-kernel` JSON with visible cell count, commit count, and durable file size.
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
git commit -m "docs: record file store status"
```
