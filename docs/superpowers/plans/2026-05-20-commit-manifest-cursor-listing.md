# Commit Manifest Cursor Listing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add cursor and limit support for commit-manifest listing so callers can consume the commit timeline incrementally.

**Architecture:** Add `CommitManifestLookup` and `KernelError::CommitNotFound` in `continuitydb-kernel`, implement cursor slicing in memory/file kernels, and expose the same lookup through `ContinuityDb`. Keep existing all-manifest listing as a default lookup convenience.

**Tech Stack:** Rust 2021, existing ContinuityDB crates, `CommitId`, `CommitManifest`, memory kernel, JSONL file kernel, TDD.

---

## File Structure

- `crates/continuitydb-kernel/src/lib.rs`: Add lookup type, error, trait method, file cursor implementation, tests.
- `crates/continuitydb-memory/src/lib.rs`: Add memory cursor implementation and tests.
- `crates/continuitydb-checkout/src/lib.rs`: Update test-only `RecordingKernel`.
- `crates/continuitydb-api/src/lib.rs`: Add API method and tests.
- `README.md`: Add current scope bullet.
- `docs/roadmap.md`: Add storage milestone.
- `docs/superpowers/plans/2026-05-20-commit-manifest-cursor-listing.md`: Track execution.

### Task 1: Kernel Cursor Lookup and Memory Behavior

**Files:**
- Modify: `crates/continuitydb-kernel/src/lib.rs`
- Modify: `crates/continuitydb-memory/src/lib.rs`
- Modify: `crates/continuitydb-checkout/src/lib.rs`

- [ ] **Step 1: Write failing memory tests**

Add in `crates/continuitydb-memory/src/lib.rs` tests:

```rust
#[test]
fn memory_kernel_lists_commit_manifests_after_cursor(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut kernel = MemoryKernel::default();
    let first_time = test_commit_time()?;
    let second_time = Utc
        .with_ymd_and_hms(2026, 5, 20, 12, 30, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let third_time = Utc
        .with_ymd_and_hms(2026, 5, 20, 13, 0, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let first_commit = CommitId::new();
    let second_commit = CommitId::new();
    let third_commit = CommitId::new();

    kernel.append_cells_at_with_commit_id(
        vec![sample_cell("project:continuitydb:cursor-first", 0.91, 12)?],
        first_time,
        first_commit,
    )?;
    kernel.append_cells_at_with_commit_id(
        vec![sample_cell("project:continuitydb:cursor-second", 0.83, 15)?],
        second_time,
        second_commit,
    )?;
    kernel.append_cells_at_with_commit_id(
        vec![sample_cell("project:continuitydb:cursor-third", 0.77, 18)?],
        third_time,
        third_commit,
    )?;

    let manifests = kernel.list_commit_manifests_matching(CommitManifestLookup {
        after: Some(first_commit),
        limit: None,
    })?;

    assert_eq!(
        manifests.iter().map(|manifest| manifest.commit_id).collect::<Vec<_>>(),
        vec![second_commit, third_commit]
    );
    Ok(())
}

#[test]
fn memory_kernel_limits_commit_manifest_listing() -> Result<(), Box<dyn std::error::Error>> {
    let mut kernel = MemoryKernel::default();
    let first_commit = CommitId::new();
    let second_commit = CommitId::new();
    kernel.append_cells_at_with_commit_id(
        vec![sample_cell("project:continuitydb:limit-first", 0.91, 12)?],
        test_commit_time()?,
        first_commit,
    )?;
    kernel.append_cells_at_with_commit_id(
        vec![sample_cell("project:continuitydb:limit-second", 0.83, 15)?],
        Utc.with_ymd_and_hms(2026, 5, 20, 12, 30, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?,
        second_commit,
    )?;

    let manifests = kernel.list_commit_manifests_matching(CommitManifestLookup {
        after: None,
        limit: Some(1),
    })?;

    assert_eq!(manifests.len(), 1);
    assert_eq!(manifests[0].commit_id, first_commit);
    Ok(())
}

#[test]
fn memory_kernel_reports_unknown_commit_manifest_cursor(
) -> Result<(), Box<dyn std::error::Error>> {
    let kernel = MemoryKernel::default();

    let result = kernel.list_commit_manifests_matching(CommitManifestLookup {
        after: Some(CommitId::new()),
        limit: None,
    });

    assert!(matches!(result, Err(KernelError::CommitNotFound)));
    Ok(())
}
```

Import `CommitManifestLookup` in the memory tests.

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p continuitydb-memory cursor
cargo test -p continuitydb-memory limits_commit_manifest
```

Expected: FAIL because `CommitManifestLookup`, `CommitNotFound`, and `list_commit_manifests_matching` do not exist.

- [ ] **Step 3: Implement kernel lookup type and memory behavior**

In `crates/continuitydb-kernel/src/lib.rs`, add:

```rust
/// Query constraints for ordered commit manifest listing.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CommitManifestLookup {
    /// Exclusive cursor commit. When present, listing starts after this commit.
    pub after: Option<CommitId>,
    /// Maximum manifests to return.
    pub limit: Option<usize>,
}
```

Add `KernelError::CommitNotFound`:

```rust
/// Requested commit was not found.
#[error("commit not found")]
CommitNotFound,
```

Add trait method:

```rust
/// Lists commit manifests matching deterministic constraints.
fn list_commit_manifests_matching(
    &self,
    lookup: CommitManifestLookup,
) -> Result<Vec<CommitManifest>, KernelError>;
```

Change `list_commit_manifests` to a default method:

```rust
fn list_commit_manifests(&self) -> Result<Vec<CommitManifest>, KernelError> {
    self.list_commit_manifests_matching(CommitManifestLookup::default())
}
```

In `crates/continuitydb-memory/src/lib.rs`, implement:

```rust
fn list_commit_manifests_matching(
    &self,
    lookup: CommitManifestLookup,
) -> Result<Vec<CommitManifest>, KernelError> {
    let start = if let Some(after) = lookup.after {
        self.manifest_order
            .iter()
            .position(|commit_id| *commit_id == after)
            .map(|position| position + 1)
            .ok_or(KernelError::CommitNotFound)?
    } else {
        0
    };
    let limit = lookup.limit.unwrap_or(usize::MAX);
    Ok(self
        .manifest_order
        .iter()
        .skip(start)
        .take(limit)
        .filter_map(|commit_id| self.manifests.get(commit_id).cloned())
        .collect())
}
```

Update `crates/continuitydb-checkout/src/lib.rs` `RecordingKernel` with:

```rust
fn list_commit_manifests_matching(
    &self,
    _lookup: continuitydb_kernel::CommitManifestLookup,
) -> Result<Vec<continuitydb_core::CommitManifest>, KernelError> {
    Ok(Vec::new())
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run:

```bash
cargo test -p continuitydb-memory cursor
cargo test -p continuitydb-memory limits_commit_manifest
```

Expected: PASS.

### Task 2: File Kernel Cursor Listing

**Files:**
- Modify: `crates/continuitydb-kernel/src/lib.rs`

- [ ] **Step 1: Write failing file test**

Add in `crates/continuitydb-kernel/src/lib.rs` tests:

```rust
#[test]
fn file_kernel_reconstructs_cursor_commit_manifest_listing_after_reopen(
) -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_kernel_path("continuitydb-file-kernel-manifest-cursor-list");
    let first_time = test_commit_time()?;
    let second_time = Utc
        .with_ymd_and_hms(2026, 5, 20, 12, 30, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let third_time = Utc
        .with_ymd_and_hms(2026, 5, 20, 13, 0, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let first_commit = CommitId::new();
    let second_commit = CommitId::new();
    let third_commit = CommitId::new();
    {
        let mut kernel = FileKernel::open(&path)?;
        kernel.append_cells_at_with_commit_id(
            vec![sample_cell("project:continuitydb:file-cursor-first", 0.91, 12)?],
            first_time,
            first_commit,
        )?;
        kernel.append_cells_at_with_commit_id(
            vec![sample_cell("project:continuitydb:file-cursor-second", 0.83, 15)?],
            second_time,
            second_commit,
        )?;
        kernel.append_cells_at_with_commit_id(
            vec![sample_cell("project:continuitydb:file-cursor-third", 0.77, 18)?],
            third_time,
            third_commit,
        )?;
    }

    let reopened = FileKernel::open(&path)?;
    let manifests = reopened.list_commit_manifests_matching(CommitManifestLookup {
        after: Some(first_commit),
        limit: Some(1),
    })?;

    assert_eq!(manifests.len(), 1);
    assert_eq!(manifests[0].commit_id, second_commit);
    fs::remove_file(path)?;
    Ok(())
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p continuitydb-kernel cursor_commit_manifest`

Expected: FAIL until file-kernel cursor listing is implemented.

- [ ] **Step 3: Implement file cursor listing**

Add `list_manifests_matching` to `FileKernelIndex` using the same cursor and limit logic as memory.

Then implement on `FileKernel`:

```rust
fn list_commit_manifests_matching(
    &self,
    lookup: CommitManifestLookup,
) -> Result<Vec<CommitManifest>, KernelError> {
    self.index.list_manifests_matching(lookup)
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p continuitydb-kernel cursor_commit_manifest`

Expected: PASS.

### Task 3: Native API Cursor Listing

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`

- [ ] **Step 1: Write failing API test**

Add in `crates/continuitydb-api/src/lib.rs` tests:

```rust
#[test]
fn api_returns_commit_manifests_after_cursor_with_limit(
) -> Result<(), Box<dyn std::error::Error>> {
    let first_time = Utc
        .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let second_time = Utc
        .with_ymd_and_hms(2026, 5, 20, 12, 30, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let third_time = Utc
        .with_ymd_and_hms(2026, 5, 20, 13, 0, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let first_commit = CommitId::new();
    let second_commit = CommitId::new();
    let third_commit = CommitId::new();
    let mut db = ContinuityDb::new(MemoryKernel::default());
    db.ingest_cells_at_with_commit_id(
        vec![sample_cell("project:continuitydb:api-cursor-first", 0.91, 12)?],
        first_time,
        first_commit,
    )?;
    db.ingest_cells_at_with_commit_id(
        vec![sample_cell("project:continuitydb:api-cursor-second", 0.83, 15)?],
        second_time,
        second_commit,
    )?;
    db.ingest_cells_at_with_commit_id(
        vec![sample_cell("project:continuitydb:api-cursor-third", 0.77, 18)?],
        third_time,
        third_commit,
    )?;

    let manifests = db.commit_manifests_matching(CommitManifestLookup {
        after: Some(first_commit),
        limit: Some(1),
    })?;

    assert_eq!(manifests.len(), 1);
    assert_eq!(manifests[0].commit_id, second_commit);
    Ok(())
}
```

Import `CommitManifestLookup` in API tests.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p continuitydb-api after_cursor`

Expected: FAIL because `ContinuityDb::commit_manifests_matching` does not exist.

- [ ] **Step 3: Implement API method**

Import `CommitManifestLookup` from `continuitydb_kernel` and add:

```rust
/// Returns commit manifests matching deterministic listing constraints.
pub fn commit_manifests_matching(
    &self,
    lookup: CommitManifestLookup,
) -> Result<Vec<CommitManifest>, ContinuityError> {
    self.kernel
        .list_commit_manifests_matching(lookup)
        .map_err(Into::into)
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p continuitydb-api after_cursor`

Expected: PASS.

### Task 4: Documentation and Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-commit-manifest-cursor-listing.md`

- [ ] **Step 1: Update docs**

In `README.md`, add:

```markdown
- Cursor-based commit manifest listing for incremental audit and sync reads.
```

In `docs/roadmap.md`, add Storage Kernel milestone 13:

```markdown
13. Add cursor-based commit manifest listing. Implemented `CommitManifestLookup` with exclusive commit cursors and limits across memory/file kernels and the native API for incremental audit, backup, and future sync reads.
```

- [ ] **Step 2: Run focused tests**

Run:

```bash
cargo test -p continuitydb-memory cursor
cargo test -p continuitydb-memory limits_commit_manifest
cargo test -p continuitydb-kernel cursor_commit_manifest
cargo test -p continuitydb-api after_cursor
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
git add crates/continuitydb-kernel/src/lib.rs crates/continuitydb-memory/src/lib.rs crates/continuitydb-checkout/src/lib.rs crates/continuitydb-api/src/lib.rs README.md docs/roadmap.md docs/superpowers/plans/2026-05-20-commit-manifest-cursor-listing.md
git commit -m "feat: add cursor commit manifest listing"
```
