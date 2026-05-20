# Atomic Write Batches Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add kernel-level atomic StateCell batch append operations and expose them through the native API.

**Architecture:** Extend `StorageKernel` with deterministic batch append methods. Implement all-or-nothing validation in `MemoryKernel` and `FileKernel`, then add ordered batch ingest wrappers in `ContinuityDb<K>`.

**Tech Stack:** Rust 2021, existing workspace crates, `continuitydb-kernel`, `continuitydb-memory`, `continuitydb-api`, TDD.

---

### Task 1: Kernel Batch Tests

**Files:**
- Modify: `crates/continuitydb-kernel/src/lib.rs`
- Modify: `crates/continuitydb-memory/src/lib.rs`

- [x] **Step 1: Write failing memory batch append test**

Add `memory_kernel_appends_batch_with_shared_system_time`.

The test should create two cells, call:

```rust
kernel.append_cells_at(vec![first.clone(), second.clone()], committed_at)?;
```

Assert lookup returns both cells and both have `system_time.from() == committed_at`.

- [x] **Step 2: Write failing memory duplicate atomicity test**

Add `memory_kernel_rejects_duplicate_ids_inside_batch_without_partial_append`.

The test should clone one cell so the batch contains the same ID twice, call `append_cells_at`, assert `KernelError::DuplicateCell`, then assert a default lookup returns no cells.

- [x] **Step 3: Write failing file batch persistence test**

Add `file_kernel_persists_batch_across_reopen_with_shared_system_time`.

The test should append two cells through `append_cells_at`, reopen the file kernel, and assert both cells are present with the same system time.

- [x] **Step 4: Write failing file duplicate atomicity tests**

Add:

```rust
file_kernel_rejects_duplicate_ids_inside_batch_without_writing_records
file_kernel_rejects_existing_ids_inside_batch_without_writing_records
```

Both tests should assert `KernelError::DuplicateCell`, reopen the file, and assert only pre-existing cells are visible.

- [x] **Step 5: Verify RED**

Run:

```bash
cargo test -p continuitydb-memory batch
cargo test -p continuitydb-kernel batch
```

Expected: FAIL because `append_cells_at` is not implemented.

### Task 2: Kernel Batch Implementation

**Files:**
- Modify: `crates/continuitydb-kernel/src/lib.rs`
- Modify: `crates/continuitydb-memory/src/lib.rs`
- Modify: `crates/continuitydb-checkout/src/lib.rs`

- [x] **Step 1: Extend `StorageKernel`**

Add `append_cells` and `append_cells_at` to the trait. Keep existing single-cell APIs and implement them in terms of batch APIs where safe.

- [x] **Step 2: Implement file kernel batch append**

Validate duplicate IDs before writing, stamp all cells with the same system time, serialize the whole batch before opening for append, write contiguous JSONL records, and update the index only after write success.

- [x] **Step 3: Implement memory kernel batch append**

Collect the batch, validate stored and in-batch duplicates before mutation, stamp all cells with the same system time, then extend the vector.

- [x] **Step 4: Update test-only trait implementors**

Update `RecordingKernel` in `continuitydb-checkout` if the new trait requires a method there.

- [x] **Step 5: Verify GREEN**

Run:

```bash
cargo test -p continuitydb-memory batch
cargo test -p continuitydb-kernel batch
cargo test -p continuitydb-checkout
```

Expected: PASS.

### Task 3: API Batch Ingest

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`

- [x] **Step 1: Write failing API batch ingest tests**

Add:

```rust
api_ingests_cell_batch_with_ordered_ids_and_shared_system_time
api_rejects_duplicate_cell_batch_without_partial_visibility
```

The first test should call `db.ingest_cells_at(vec![first, second], committed_at)` and assert returned IDs preserve input order and both stored cells share `committed_at`.

The second test should pass duplicate IDs, assert `ContinuityError::Kernel(KernelError::DuplicateCell)`, and assert neither cell is visible.

- [x] **Step 2: Verify RED**

Run:

```bash
cargo test -p continuitydb-api batch_ingest
```

Expected: FAIL because API batch ingest methods are not implemented.

- [x] **Step 3: Implement API batch ingest methods**

Add:

```rust
pub fn ingest_cells<I>(&mut self, cells: I) -> Result<Vec<StateCellId>, ContinuityError>
where
    I: IntoIterator<Item = StateCell>

pub fn ingest_cells_at<I>(
    &mut self,
    cells: I,
    committed_at: DateTime<Utc>,
) -> Result<Vec<StateCellId>, ContinuityError>
where
    I: IntoIterator<Item = StateCell>
```

Collect cells once, preserve ordered IDs, call `self.kernel.append_cells_at(cells, committed_at)`, then return the IDs.

- [x] **Step 4: Verify GREEN**

Run:

```bash
cargo test -p continuitydb-api batch_ingest
cargo test -p continuitydb-api
```

Expected: PASS.

### Task 4: Roadmap, Full Verification, Commit

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-atomic-write-batches.md`

- [x] **Step 1: Update docs**

Add atomic write batches to README current scope and Storage Kernel Milestones.

- [x] **Step 2: Run full verification**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

Expected: all commands pass.

- [x] **Step 3: Commit**

Run:

```bash
git add crates/continuitydb-kernel crates/continuitydb-memory crates/continuitydb-checkout crates/continuitydb-api README.md docs/roadmap.md docs/superpowers/plans/2026-05-20-atomic-write-batches.md
git commit -m "feat: add atomic write batches"
```
