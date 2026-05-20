# Commit Manifests Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add first-class commit manifests that expose which cells were written by a commit boundary and when that boundary was committed.

**Architecture:** Add a minimal `CommitManifest` domain type in `continuitydb-core`, extend `StorageKernel` with manifest lookup, and have memory/file kernels record or reconstruct manifests from successful non-empty batches. Expose manifest lookup through `ContinuityDb` and document the new roadmap milestone.

**Tech Stack:** Rust 2021, existing workspace crates, `chrono`, `serde`, append-only JSONL file kernel, TDD.

---

## File Structure

- `crates/continuitydb-core/src/cell.rs`: Add `CommitManifest`.
- `crates/continuitydb-core/src/lib.rs`: Export `CommitManifest`.
- `crates/continuitydb-kernel/src/lib.rs`: Import `CommitManifest`, extend `KernelError`, `StorageKernel`, `FileKernelIndex`, and file-kernel tests.
- `crates/continuitydb-memory/src/lib.rs`: Store manifests in memory and add memory-kernel tests.
- `crates/continuitydb-api/src/lib.rs`: Expose `ContinuityDb::commit_manifest` and add API tests.
- `README.md`: Add current scope bullet.
- `docs/roadmap.md`: Add storage milestone for commit manifests.
- `docs/superpowers/plans/2026-05-20-commit-manifests.md`: Track implementation.

### Task 1: Core Commit Manifest Type

**Files:**
- Modify: `crates/continuitydb-core/src/cell.rs`
- Modify: `crates/continuitydb-core/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Add this test in `crates/continuitydb-core/src/cell.rs` tests:

```rust
#[test]
fn commit_manifest_records_commit_boundary_and_ordered_cells(
) -> Result<(), Box<dyn std::error::Error>> {
    let committed_at = chrono::Utc
        .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let commit_id = CommitId::new();
    let first = StateCellId::new();
    let second = StateCellId::new();

    let manifest = CommitManifest::new(commit_id, committed_at, vec![first, second]);

    assert_eq!(manifest.commit_id, commit_id);
    assert_eq!(manifest.committed_at, committed_at);
    assert_eq!(manifest.cell_ids, vec![first, second]);
    Ok(())
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p continuitydb-core commit_manifest_records_commit_boundary_and_ordered_cells`

Expected: FAIL because `CommitManifest` is not defined.

- [ ] **Step 3: Write minimal implementation**

Add near `CommitId` in `crates/continuitydb-core/src/cell.rs`:

```rust
/// Immutable record of a database commit boundary.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CommitManifest {
    /// Commit identifier shared by all cells written in this boundary.
    pub commit_id: CommitId,
    /// System transaction time assigned to the commit.
    pub committed_at: chrono::DateTime<chrono::Utc>,
    /// StateCell identifiers written by this commit in append order.
    pub cell_ids: Vec<StateCellId>,
}

impl CommitManifest {
    /// Creates a commit manifest.
    pub fn new(
        commit_id: CommitId,
        committed_at: chrono::DateTime<chrono::Utc>,
        cell_ids: Vec<StateCellId>,
    ) -> Self {
        Self {
            commit_id,
            committed_at,
            cell_ids,
        }
    }
}
```

Export it from `crates/continuitydb-core/src/lib.rs` in the existing `pub use cell::{ ... }` list.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p continuitydb-core commit_manifest_records_commit_boundary_and_ordered_cells`

Expected: PASS.

### Task 2: Kernel Manifest Contract

**Files:**
- Modify: `crates/continuitydb-kernel/src/lib.rs`
- Modify: `crates/continuitydb-memory/src/lib.rs`

- [ ] **Step 1: Write failing memory-kernel tests**

Add tests in `crates/continuitydb-memory/src/lib.rs`:

```rust
#[test]
fn memory_kernel_records_commit_manifest_for_batch() -> Result<(), Box<dyn std::error::Error>> {
    let mut kernel = MemoryKernel::default();
    let committed_at = test_commit_time()?;
    let commit_id = CommitId::new();
    let first = sample_cell("project:continuitydb:manifest-first", 0.91, 12)?;
    let second = sample_cell("project:continuitydb:manifest-second", 0.83, 15)?;
    let expected_ids = vec![first.id, second.id];

    kernel.append_cells_at_with_commit_id(vec![first, second], committed_at, commit_id)?;

    let manifest = kernel
        .lookup_commit_manifest(commit_id)?
        .ok_or_else(|| std::io::Error::other("missing manifest"))?;
    assert_eq!(manifest.commit_id, commit_id);
    assert_eq!(manifest.committed_at, committed_at);
    assert_eq!(manifest.cell_ids, expected_ids);
    Ok(())
}

#[test]
fn memory_kernel_rejects_duplicate_commit_id_without_partial_visibility(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut kernel = MemoryKernel::default();
    let committed_at = test_commit_time()?;
    let commit_id = CommitId::new();
    let first = sample_cell("project:continuitydb:manifest-first", 0.91, 12)?;
    let second = sample_cell("project:continuitydb:manifest-second", 0.83, 15)?;

    kernel.append_cells_at_with_commit_id(vec![first.clone()], committed_at, commit_id)?;
    let result = kernel.append_cells_at_with_commit_id(vec![second], committed_at, commit_id);

    assert!(matches!(result, Err(KernelError::DuplicateCommit)));
    let stored = kernel.lookup_cells(CellLookup::default())?;
    assert_eq!(stored.iter().map(|cell| cell.id).collect::<Vec<_>>(), vec![first.id]);
    assert_eq!(
        kernel
            .lookup_commit_manifest(commit_id)?
            .ok_or_else(|| std::io::Error::other("missing manifest"))?
            .cell_ids,
        vec![first.id]
    );
    Ok(())
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p continuitydb-memory commit_manifest`

Expected: FAIL because `lookup_commit_manifest` and `DuplicateCommit` do not exist.

- [ ] **Step 3: Extend kernel trait and memory implementation**

In `crates/continuitydb-kernel/src/lib.rs`, import `CommitManifest`, add `DuplicateCommit`, and add the trait method:

```rust
/// A commit identifier already has a visible manifest.
#[error("commit already exists")]
DuplicateCommit,
```

```rust
/// Looks up a commit manifest by commit ID.
fn lookup_commit_manifest(
    &self,
    commit_id: CommitId,
) -> Result<Option<CommitManifest>, KernelError>;
```

In `crates/continuitydb-memory/src/lib.rs`, add a manifest map:

```rust
#[derive(Default)]
pub struct MemoryKernel {
    cells: Vec<StateCell>,
    manifests: std::collections::HashMap<CommitId, CommitManifest>,
}
```

Then update successful non-empty append:

```rust
if !stamped.is_empty() && self.manifests.contains_key(&commit_id) {
    return Err(KernelError::DuplicateCommit);
}
let cell_ids = stamped.iter().map(|cell| cell.id).collect::<Vec<_>>();
self.cells.extend(stamped);
if !cell_ids.is_empty() {
    self.manifests.insert(
        commit_id,
        CommitManifest::new(commit_id, committed_at, cell_ids),
    );
}
```

Add:

```rust
fn lookup_commit_manifest(
    &self,
    commit_id: CommitId,
) -> Result<Option<CommitManifest>, KernelError> {
    Ok(self.manifests.get(&commit_id).cloned())
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p continuitydb-memory commit_manifest`

Expected: PASS.

### Task 3: File Kernel Manifest Reconstruction

**Files:**
- Modify: `crates/continuitydb-kernel/src/lib.rs`

- [ ] **Step 1: Write failing file-kernel tests**

Add tests in `crates/continuitydb-kernel/src/lib.rs`:

```rust
#[test]
fn file_kernel_reconstructs_commit_manifest_after_reopen(
) -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_kernel_path("continuitydb-file-kernel-manifest");
    let committed_at = test_commit_time()?;
    let commit_id = CommitId::new();
    let first = sample_cell("project:continuitydb:manifest-first", 0.91, 12)?;
    let second = sample_cell("project:continuitydb:manifest-second", 0.83, 15)?;
    let expected_ids = vec![first.id, second.id];
    {
        let mut kernel = FileKernel::open(&path)?;
        kernel.append_cells_at_with_commit_id(vec![first, second], committed_at, commit_id)?;
    }

    let reopened = FileKernel::open(&path)?;
    let manifest = reopened
        .lookup_commit_manifest(commit_id)?
        .ok_or_else(|| std::io::Error::other("missing manifest"))?;

    assert_eq!(manifest.commit_id, commit_id);
    assert_eq!(manifest.committed_at, committed_at);
    assert_eq!(manifest.cell_ids, expected_ids);
    fs::remove_file(path)?;
    Ok(())
}

#[test]
fn file_kernel_rejects_duplicate_commit_id_without_writing_records(
) -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_kernel_path("continuitydb-file-kernel-duplicate-manifest");
    let committed_at = test_commit_time()?;
    let commit_id = CommitId::new();
    let first = sample_cell("project:continuitydb:manifest-first", 0.91, 12)?;
    let second = sample_cell("project:continuitydb:manifest-second", 0.83, 15)?;
    {
        let mut kernel = FileKernel::open(&path)?;
        kernel.append_cells_at_with_commit_id(vec![first.clone()], committed_at, commit_id)?;
        let result = kernel.append_cells_at_with_commit_id(vec![second], committed_at, commit_id);
        assert!(matches!(result, Err(KernelError::DuplicateCommit)));
    }

    let reopened = FileKernel::open(&path)?;
    let stored = reopened.lookup_cells(CellLookup::default())?;
    assert_eq!(stored.iter().map(|cell| cell.id).collect::<Vec<_>>(), vec![first.id]);
    assert_eq!(
        reopened
            .lookup_commit_manifest(commit_id)?
            .ok_or_else(|| std::io::Error::other("missing manifest"))?
            .cell_ids,
        vec![first.id]
    );
    fs::remove_file(path)?;
    Ok(())
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p continuitydb-kernel commit_manifest`

Expected: FAIL because file kernel does not store manifests.

- [ ] **Step 3: Implement file-kernel manifest index**

Add `manifests: HashMap<CommitId, CommitManifest>` to `FileKernelIndex`. In `insert`, create or extend the manifest for the cell's `commit_id` using `cell.system_time.from()` and append order. Before writing a new non-empty batch, reject `commit_id` if a manifest already exists. After writing, insert stamped cells as today.

Add this helper to `FileKernelIndex`:

```rust
fn manifest_by_id(&self, commit_id: CommitId) -> Option<CommitManifest> {
    self.manifests.get(&commit_id).cloned()
}
```

Implement `StorageKernel::lookup_commit_manifest` for `FileKernel` by returning `self.index.manifest_by_id(commit_id)`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p continuitydb-kernel commit_manifest`

Expected: PASS.

### Task 4: Native API Manifest Lookup

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`

- [ ] **Step 1: Write failing API tests**

Add tests in `crates/continuitydb-api/src/lib.rs`:

```rust
#[test]
fn api_returns_commit_manifest_for_committed_batch() -> Result<(), Box<dyn std::error::Error>> {
    let committed_at = Utc
        .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let commit_id = CommitId::new();
    let mut db = ContinuityDb::new(MemoryKernel::default());
    let first = sample_cell("project:continuitydb:manifest-first", 0.91, 12)?;
    let second = sample_cell("project:continuitydb:manifest-second", 0.83, 15)?;
    let expected_ids = vec![first.id, second.id];

    db.ingest_cells_at_with_commit_id(vec![first, second], committed_at, commit_id)?;

    let manifest = db
        .commit_manifest(commit_id)?
        .ok_or_else(|| std::io::Error::other("missing manifest"))?;
    assert_eq!(manifest.commit_id, commit_id);
    assert_eq!(manifest.committed_at, committed_at);
    assert_eq!(manifest.cell_ids, expected_ids);
    Ok(())
}

#[test]
fn api_returns_none_for_unknown_commit_manifest() -> Result<(), Box<dyn std::error::Error>> {
    let db = ContinuityDb::new(MemoryKernel::default());

    assert!(db.commit_manifest(CommitId::new())?.is_none());
    Ok(())
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p continuitydb-api commit_manifest`

Expected: FAIL because `ContinuityDb::commit_manifest` does not exist.

- [ ] **Step 3: Implement API method**

Import `CommitManifest` from `continuitydb_core` and add:

```rust
/// Returns the manifest for a database commit boundary when it exists.
pub fn commit_manifest(
    &self,
    commit_id: CommitId,
) -> Result<Option<CommitManifest>, ContinuityError> {
    self.kernel
        .lookup_commit_manifest(commit_id)
        .map_err(Into::into)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p continuitydb-api commit_manifest`

Expected: PASS.

### Task 5: Documentation and Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-commit-manifests.md`

- [ ] **Step 1: Update docs**

In `README.md`, add:

```markdown
- First-class commit manifests for transaction-boundary inspection.
```

In `docs/roadmap.md`, add Storage Kernel milestone 11:

```markdown
11. Add first-class commit manifests. Implemented `CommitManifest` and kernel/API manifest lookup so a commit boundary can expose its commit time and ordered StateCell IDs without reconstructing from checkout results.
```

- [ ] **Step 2: Run focused tests**

Run:

```bash
cargo test -p continuitydb-core commit_manifest
cargo test -p continuitydb-memory commit_manifest
cargo test -p continuitydb-kernel commit_manifest
cargo test -p continuitydb-api commit_manifest
```

Expected: all PASS.

- [ ] **Step 3: Run full verification gate**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

Expected: all PASS.

- [ ] **Step 4: Commit**

Run:

```bash
git add crates/continuitydb-core/src/cell.rs crates/continuitydb-core/src/lib.rs crates/continuitydb-kernel/src/lib.rs crates/continuitydb-memory/src/lib.rs crates/continuitydb-api/src/lib.rs README.md docs/roadmap.md docs/superpowers/plans/2026-05-20-commit-manifests.md
git commit -m "feat: add commit manifests"
```
