# Native Operation API Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the first typed embeddable ContinuityDB operation API for ingest, checkout, and audit.

**Architecture:** Extend `CellLookup` with immutable cell-ID lookup and implement it in memory and file kernels. Add a new `continuitydb-api` crate with `ContinuityDb<K>` as a generic wrapper over `StorageKernel`, delegating checkout to `continuitydb-checkout` and audit to stored-cell lookup plus `audit`.

**Tech Stack:** Rust 2021, existing workspace crates, `thiserror`, TDD.

---

### Task 1: First-Class Cell ID Lookup

**Files:**
- Modify: `crates/continuitydb-kernel/src/lib.rs`
- Modify: `crates/continuitydb-memory/src/lib.rs`

- [x] **Step 1: Write failing memory and file kernel tests**

Add tests named:

```rust
memory_kernel_filters_by_cell_id
file_kernel_filters_by_cell_id
```

Each test appends two cells and looks up the second cell with:

```rust
CellLookup {
    cell_id: Some(second.id),
    ..CellLookup::default()
}
```

Expected result is exactly `vec![second]`.

- [x] **Step 2: Verify RED**

Run:

```bash
cargo test -p continuitydb-memory memory_kernel_filters_by_cell_id
cargo test -p continuitydb-kernel file_kernel_filters_by_cell_id
```

Expected: FAIL because `CellLookup::cell_id` is not implemented.

- [x] **Step 3: Implement cell ID lookup**

Add:

```rust
pub cell_id: Option<StateCellId>,
```

to `CellLookup`. In `MemoryKernel`, filter by `cell.id == id`. In `FileKernel`, use the existing ID index to return the matching indexed cell when `cell_id` is supplied.

- [x] **Step 4: Verify GREEN**

Run:

```bash
cargo test -p continuitydb-memory memory_kernel_filters_by_cell_id
cargo test -p continuitydb-kernel file_kernel_filters_by_cell_id
```

Expected: PASS.

### Task 2: Native API Crate

**Files:**
- Modify: `Cargo.toml`
- Create: `crates/continuitydb-api/Cargo.toml`
- Create: `crates/continuitydb-api/src/lib.rs`

- [x] **Step 1: Write failing API tests**

Create tests in `crates/continuitydb-api/src/lib.rs`:

```rust
api_ingests_and_checkouts_cells
api_audits_cell_by_id_with_structured_evidence
api_audit_cell_reports_missing_id
```

The tests should use `MemoryKernel`, append deterministic sample cells through `ContinuityDb`, and assert checkout/audit behavior.

- [x] **Step 2: Verify RED**

Run:

```bash
cargo test -p continuitydb-api
```

Expected: FAIL because the crate and API are not implemented.

- [x] **Step 3: Implement API crate**

Add `ContinuityDb<K>` with:

```rust
pub fn new(kernel: K) -> Self
pub fn kernel(&self) -> &K
pub fn kernel_mut(&mut self) -> &mut K
pub fn into_kernel(self) -> K
pub fn ingest_cell(&mut self, cell: StateCell) -> Result<StateCellId, ContinuityError>
pub fn ingest_cell_at(
    &mut self,
    cell: StateCell,
    committed_at: DateTime<Utc>,
) -> Result<StateCellId, ContinuityError>
pub fn checkout(&self, request: CheckoutRequest) -> Result<CheckoutSlice, ContinuityError>
pub fn audit_cell(&self, cell_id: StateCellId) -> Result<AuditTrace, ContinuityError>
```

Add `ContinuityError::{Kernel, Checkout, CellNotFound}`.

- [x] **Step 4: Verify GREEN**

Run:

```bash
cargo test -p continuitydb-api
```

Expected: PASS.

### Task 3: Roadmap and Workspace Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-native-operation-api.md`

- [x] **Step 1: Update docs**

Add a Native API milestone to `docs/roadmap.md` and add the API crate to README current scope.

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
git add Cargo.toml crates/continuitydb-api crates/continuitydb-kernel/src/lib.rs crates/continuitydb-memory/src/lib.rs README.md docs/roadmap.md docs/superpowers/specs/2026-05-20-native-operation-api-design.md docs/superpowers/plans/2026-05-20-native-operation-api.md
git commit -m "feat: add native operation api"
```
