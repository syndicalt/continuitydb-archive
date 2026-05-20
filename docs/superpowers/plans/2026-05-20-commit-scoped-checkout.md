# Commit-Scoped Checkout Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add commit-ID constraints to deterministic checkout so applications can materialize transaction-scoped continuity slices.

**Architecture:** Extend `CheckoutRequest` with `commit_id: Option<CommitId>` and pass it into `CellLookup`. Use the existing memory/file kernel commit-ID filtering; do not duplicate filtering in checkout.

**Tech Stack:** Rust 2021, existing workspace crates, `continuitydb-checkout`, TDD.

---

### Task 1: Commit-Scoped Checkout Tests

**Files:**
- Modify: `crates/continuitydb-checkout/src/lib.rs`

- [x] **Step 1: Write failing pushdown test**

Add `checkout_pushes_commit_id_to_kernel`. Use `RecordingKernel`, set `commit_id: Some(commit_id)` in `CheckoutRequest`, run `checkout`, then assert the recorded `CellLookup.commit_id == Some(commit_id)`.

- [x] **Step 2: Write failing behavior test**

Add `checkout_filters_by_commit_id`. Append two cells with two different explicit commit IDs through `MemoryKernel::append_cell_at_with_commit_id`. Request checkout with the first commit ID and assert only the first cell is returned.

- [x] **Step 3: Verify RED**

Run:

```bash
cargo test -p continuitydb-checkout commit_id
```

Expected: FAIL because `CheckoutRequest.commit_id` does not exist.

### Task 2: Commit-Scoped Checkout Implementation

**Files:**
- Modify: `crates/continuitydb-checkout/src/lib.rs`
- Modify any crate tests or binaries that construct `CheckoutRequest`.

- [x] **Step 1: Add `commit_id` to `CheckoutRequest`**

Import `CommitId` from `continuitydb_core` and add:

```rust
pub commit_id: Option<CommitId>
```

- [x] **Step 2: Push `commit_id` into `CellLookup`**

Set:

```rust
commit_id: request.commit_id,
```

in the `CellLookup` constructed by `checkout`.

- [x] **Step 3: Update existing request literals**

Add `commit_id: None` to all existing `CheckoutRequest` literals in workspace tests and binaries.

- [x] **Step 4: Verify GREEN**

Run:

```bash
cargo test -p continuitydb-checkout commit_id
cargo test -p continuitydb-checkout
```

Expected: PASS.

### Task 3: Docs, Full Verification, Commit

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-commit-scoped-checkout.md`

- [x] **Step 1: Update docs**

Add commit-scoped checkout to README current scope and Checkout Milestones.

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
git add crates README.md docs/roadmap.md docs/superpowers/plans/2026-05-20-commit-scoped-checkout.md
git commit -m "feat: add commit-scoped checkout"
```
