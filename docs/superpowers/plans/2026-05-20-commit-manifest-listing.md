# Commit Manifest Listing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add commit-manifest listing so callers can inspect the ordered database commit timeline.

**Architecture:** Extend `StorageKernel` with `list_commit_manifests`, keep visibility order in memory/file index state, and expose the ordered list through `ContinuityDb`. The file kernel reconstructs ordering from first-seen commit IDs in the JSONL cell log.

**Tech Stack:** Rust 2021, existing ContinuityDB crates, `CommitManifest`, memory kernel, JSONL file kernel, TDD.

---

## File Structure

- `crates/continuitydb-kernel/src/lib.rs`: Extend trait, file index ordering, file-kernel tests.
- `crates/continuitydb-memory/src/lib.rs`: Store manifest order, add memory tests.
- `crates/continuitydb-checkout/src/lib.rs`: Update test-only `RecordingKernel`.
- `crates/continuitydb-api/src/lib.rs`: Add native API method and tests.
- `README.md`: Add current scope bullet.
- `docs/roadmap.md`: Add storage milestone.
- `docs/superpowers/plans/2026-05-20-commit-manifest-listing.md`: Track execution.

### Task 1: Memory Kernel Listing

**Files:**
- Modify: `crates/continuitydb-kernel/src/lib.rs`
- Modify: `crates/continuitydb-memory/src/lib.rs`
- Modify: `crates/continuitydb-checkout/src/lib.rs`

- [ ] **Step 1: Write failing memory tests**

Add in `crates/continuitydb-memory/src/lib.rs` tests:

```rust
#[test]
fn memory_kernel_lists_commit_manifests_in_append_order(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut kernel = MemoryKernel::default();
    let first_time = test_commit_time()?;
    let second_time = Utc
        .with_ymd_and_hms(2026, 5, 20, 12, 30, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let first_commit = CommitId::new();
    let second_commit = CommitId::new();
    let first = sample_cell("project:continuitydb:list-first", 0.91, 12)?;
    let second = sample_cell("project:continuitydb:list-second", 0.83, 15)?;
    let expected_first_ids = vec![first.id];
    let expected_second_ids = vec![second.id];

    kernel.append_cells_at_with_commit_id(vec![first], first_time, first_commit)?;
    kernel.append_cells_at_with_commit_id(vec![second], second_time, second_commit)?;

    let manifests = kernel.list_commit_manifests()?;
    assert_eq!(manifests.len(), 2);
    assert_eq!(manifests[0].commit_id, first_commit);
    assert_eq!(manifests[0].committed_at, first_time);
    assert_eq!(manifests[0].cell_ids, expected_first_ids);
    assert_eq!(manifests[1].commit_id, second_commit);
    assert_eq!(manifests[1].committed_at, second_time);
    assert_eq!(manifests[1].cell_ids, expected_second_ids);
    Ok(())
}

#[test]
fn memory_kernel_omits_empty_batches_from_commit_manifest_listing(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut kernel = MemoryKernel::default();

    kernel.append_cells_at_with_commit_id(Vec::new(), test_commit_time()?, CommitId::new())?;

    assert!(kernel.list_commit_manifests()?.is_empty());
    Ok(())
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p continuitydb-memory list_commit_manifests`

Expected: FAIL because `list_commit_manifests` does not exist.

- [ ] **Step 3: Extend trait and memory implementation**

In `crates/continuitydb-kernel/src/lib.rs`, add to `StorageKernel`:

```rust
/// Lists commit manifests in commit visibility order.
fn list_commit_manifests(&self) -> Result<Vec<CommitManifest>, KernelError>;
```

In `crates/continuitydb-memory/src/lib.rs`, add `manifest_order`:

```rust
pub struct MemoryKernel {
    cells: Vec<StateCell>,
    manifests: HashMap<CommitId, CommitManifest>,
    manifest_order: Vec<CommitId>,
}
```

When a non-empty batch succeeds, push `commit_id` after inserting the manifest:

```rust
self.manifest_order.push(commit_id);
```

Add:

```rust
fn list_commit_manifests(&self) -> Result<Vec<CommitManifest>, KernelError> {
    Ok(self
        .manifest_order
        .iter()
        .filter_map(|commit_id| self.manifests.get(commit_id).cloned())
        .collect())
}
```

In `crates/continuitydb-checkout/src/lib.rs`, update `RecordingKernel` with:

```rust
fn list_commit_manifests(
    &self,
) -> Result<Vec<continuitydb_core::CommitManifest>, KernelError> {
    Ok(Vec::new())
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p continuitydb-memory list_commit_manifests`

Expected: PASS.

### Task 2: File Kernel Listing

**Files:**
- Modify: `crates/continuitydb-kernel/src/lib.rs`

- [ ] **Step 1: Write failing file tests**

Add in `crates/continuitydb-kernel/src/lib.rs` tests:

```rust
#[test]
fn file_kernel_reconstructs_commit_manifest_listing_after_reopen(
) -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_kernel_path("continuitydb-file-kernel-manifest-list");
    let first_time = test_commit_time()?;
    let second_time = Utc
        .with_ymd_and_hms(2026, 5, 20, 12, 30, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let first_commit = CommitId::new();
    let second_commit = CommitId::new();
    let first = sample_cell("project:continuitydb:list-first", 0.91, 12)?;
    let second = sample_cell("project:continuitydb:list-second", 0.83, 15)?;
    let expected_first_ids = vec![first.id];
    let expected_second_ids = vec![second.id];
    {
        let mut kernel = FileKernel::open(&path)?;
        kernel.append_cells_at_with_commit_id(vec![first], first_time, first_commit)?;
        kernel.append_cells_at_with_commit_id(vec![second], second_time, second_commit)?;
    }

    let reopened = FileKernel::open(&path)?;
    let manifests = reopened.list_commit_manifests()?;

    assert_eq!(manifests.len(), 2);
    assert_eq!(manifests[0].commit_id, first_commit);
    assert_eq!(manifests[0].committed_at, first_time);
    assert_eq!(manifests[0].cell_ids, expected_first_ids);
    assert_eq!(manifests[1].commit_id, second_commit);
    assert_eq!(manifests[1].committed_at, second_time);
    assert_eq!(manifests[1].cell_ids, expected_second_ids);
    fs::remove_file(path)?;
    Ok(())
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p continuitydb-kernel list_commit_manifests`

Expected: FAIL until file index listing is implemented.

- [ ] **Step 3: Implement file index ordering**

Add `manifest_order: Vec<CommitId>` to `FileKernelIndex`.

In `insert`, when the commit does not already exist, push it before creating the manifest:

```rust
if !self.manifests.contains_key(&cell.commit_id) {
    self.manifest_order.push(cell.commit_id);
}
```

Add:

```rust
fn list_manifests(&self) -> Vec<CommitManifest> {
    self.manifest_order
        .iter()
        .filter_map(|commit_id| self.manifests.get(commit_id).cloned())
        .collect()
}
```

Implement on `FileKernel`:

```rust
fn list_commit_manifests(&self) -> Result<Vec<CommitManifest>, KernelError> {
    Ok(self.index.list_manifests())
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p continuitydb-kernel list_commit_manifests`

Expected: PASS.

### Task 3: Native API Listing

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`

- [ ] **Step 1: Write failing API test**

Add in `crates/continuitydb-api/src/lib.rs` tests:

```rust
#[test]
fn api_returns_commit_manifests_in_kernel_order() -> Result<(), Box<dyn std::error::Error>> {
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
    let mut db = ContinuityDb::new(MemoryKernel::default());
    let first = sample_cell("project:continuitydb:list-first", 0.91, 12)?;
    let second = sample_cell("project:continuitydb:list-second", 0.83, 15)?;

    db.ingest_cells_at_with_commit_id(vec![first], first_time, first_commit)?;
    db.ingest_cells_at_with_commit_id(vec![second], second_time, second_commit)?;

    let manifests = db.commit_manifests()?;
    assert_eq!(
        manifests.iter().map(|manifest| manifest.commit_id).collect::<Vec<_>>(),
        vec![first_commit, second_commit]
    );
    assert_eq!(manifests[0].committed_at, first_time);
    assert_eq!(manifests[1].committed_at, second_time);
    Ok(())
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p continuitydb-api commit_manifests`

Expected: FAIL because `ContinuityDb::commit_manifests` does not exist.

- [ ] **Step 3: Implement API method**

Add:

```rust
/// Returns commit manifests in database visibility order.
pub fn commit_manifests(&self) -> Result<Vec<CommitManifest>, ContinuityError> {
    self.kernel.list_commit_manifests().map_err(Into::into)
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p continuitydb-api commit_manifests`

Expected: PASS.

### Task 4: Documentation and Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-commit-manifest-listing.md`

- [ ] **Step 1: Update docs**

In `README.md`, add:

```markdown
- Commit manifest timeline listing.
```

In `docs/roadmap.md`, add Storage Kernel milestone 12:

```markdown
12. Add commit manifest timeline listing. Implemented ordered `list_commit_manifests` support across the storage kernel, memory/file kernels, and native API so audit and sync callers can discover commit boundaries deterministically.
```

- [ ] **Step 2: Run focused tests**

Run:

```bash
cargo test -p continuitydb-memory list_commit_manifests
cargo test -p continuitydb-kernel list_commit_manifests
cargo test -p continuitydb-api commit_manifests
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
git add crates/continuitydb-kernel/src/lib.rs crates/continuitydb-memory/src/lib.rs crates/continuitydb-checkout/src/lib.rs crates/continuitydb-api/src/lib.rs README.md docs/roadmap.md docs/superpowers/plans/2026-05-20-commit-manifest-listing.md
git commit -m "feat: list commit manifests"
```
