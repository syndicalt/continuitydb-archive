# Commit Identifiers Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add first-class commit IDs so cells written together can be audited and queried as one commit boundary.

**Architecture:** Add `CommitId` to `continuitydb-core` and `StateCell`. Extend `StorageKernel` append paths to stamp commit IDs, add `CellLookup.commit_id`, and expose explicit deterministic commit IDs through `ContinuityDb`.

**Tech Stack:** Rust 2021, existing workspace crates, `uuid`, `serde`, TDD.

---

### Task 1: Core Commit ID

**Files:**
- Modify: `crates/continuitydb-core/src/cell.rs`
- Modify: `crates/continuitydb-core/src/lib.rs`

- [x] **Step 1: Write failing core tests**

Add tests:

```rust
state_cell_starts_with_nil_commit_id
state_cell_deserializes_missing_commit_id_as_nil
commit_id_new_is_not_nil
```

- [x] **Step 2: Verify RED**

Run:

```bash
cargo test -p continuitydb-core commit_id
```

Expected: FAIL because `CommitId` and `StateCell.commit_id` do not exist.

- [x] **Step 3: Implement `CommitId`**

Add UUID-backed `CommitId` with `new`, `nil`, `is_nil`, `Default`, serde derives, and public re-export.

- [x] **Step 4: Add `StateCell.commit_id`**

Add `#[serde(default)] pub commit_id: CommitId` to `StateCell`, initialized with `CommitId::nil()` in `StateCell::new`.

- [x] **Step 5: Verify GREEN**

Run:

```bash
cargo test -p continuitydb-core commit_id
cargo test -p continuitydb-core
```

Expected: PASS.

### Task 2: Kernel Commit Stamping And Lookup

**Files:**
- Modify: `crates/continuitydb-kernel/src/lib.rs`
- Modify: `crates/continuitydb-memory/src/lib.rs`
- Modify: `crates/continuitydb-checkout/src/lib.rs`

- [x] **Step 1: Write failing kernel tests**

Add tests proving memory and file kernels can append a batch with explicit commit ID and retrieve it through `CellLookup { commit_id: Some(commit_id), .. }`.

- [x] **Step 2: Verify RED**

Run:

```bash
cargo test -p continuitydb-memory commit_id
cargo test -p continuitydb-kernel commit_id
```

Expected: FAIL because commit lookup and explicit commit append methods do not exist.

- [x] **Step 3: Extend storage trait and lookup**

Add `commit_id: Option<CommitId>` to `CellLookup`. Add:

```rust
append_cell_at_with_commit_id
append_cells_at_with_commit_id
```

Default `append_cell_at` and `append_cells_at` should generate a fresh `CommitId`.

- [x] **Step 4: Implement memory and file behavior**

Stamp every accepted cell with the supplied commit ID. Add file-kernel commit index rebuild/insert support and filtering in both kernels.

- [x] **Step 5: Update test-only implementors**

Update `RecordingKernel` in checkout tests for the new trait method if needed.

- [x] **Step 6: Verify GREEN**

Run:

```bash
cargo test -p continuitydb-memory commit_id
cargo test -p continuitydb-kernel commit_id
cargo test -p continuitydb-checkout
```

Expected: PASS.

### Task 3: API Commit IDs

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`

- [x] **Step 1: Write failing API test**

Add `api_batch_ingest_at_with_commit_id_stamps_lookup_boundary`.

The test should call:

```rust
db.ingest_cells_at_with_commit_id(vec![first, second], committed_at, commit_id)?;
```

Then assert lookup by `CellLookup.commit_id` returns both cells with the supplied commit ID.

- [x] **Step 2: Verify RED**

Run:

```bash
cargo test -p continuitydb-api commit_id
```

Expected: FAIL because API explicit commit methods do not exist.

- [x] **Step 3: Implement API methods**

Add `ingest_cell_at_with_commit_id` and `ingest_cells_at_with_commit_id`, preserving ordered returned IDs.

- [x] **Step 4: Verify GREEN**

Run:

```bash
cargo test -p continuitydb-api commit_id
cargo test -p continuitydb-api
```

Expected: PASS.

### Task 4: Docs, Full Verification, Commit

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-commit-identifiers.md`

- [x] **Step 1: Update docs**

Add commit identifiers to README current scope and Storage Kernel Milestones.

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
git add crates/continuitydb-core crates/continuitydb-kernel crates/continuitydb-memory crates/continuitydb-checkout crates/continuitydb-api README.md docs/roadmap.md docs/superpowers/plans/2026-05-20-commit-identifiers.md
git commit -m "feat: add commit identifiers"
```
