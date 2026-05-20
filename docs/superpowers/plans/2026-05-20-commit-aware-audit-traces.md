# Commit-Aware Audit Traces Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add commit IDs to checkout/audit traces so audited StateCells expose their database commit boundary.

**Architecture:** Extend `AuditTrace` with `commit_id: CommitId` and populate it inside `audit(cell)`. Checkout already derives audit traces through `audit`, so selected checkout cells inherit the metadata automatically.

**Tech Stack:** Rust 2021, existing workspace crates, `continuitydb-checkout`, TDD.

---

### Task 1: Commit-Aware Audit Tests

**Files:**
- Modify: `crates/continuitydb-checkout/src/lib.rs`

- [x] **Step 1: Write failing direct audit test**

Add `audit_includes_commit_id`. Create a sample cell, set `cell.commit_id = CommitId::new()`, call `audit(&cell)`, and assert `trace.commit_id == cell.commit_id`.

- [x] **Step 2: Write failing checkout audit test**

Add `checkout_audit_traces_include_commit_ids`. Append a cell through `append_committed`, run checkout, and assert `slice.audit_traces[0].commit_id == selected.commit_id`.

- [x] **Step 3: Verify RED**

Run:

```bash
cargo test -p continuitydb-checkout commit_id
```

Expected: FAIL because `AuditTrace.commit_id` does not exist.

### Task 2: Audit Trace Implementation

**Files:**
- Modify: `crates/continuitydb-checkout/src/lib.rs`

- [x] **Step 1: Add `commit_id` field**

Add `CommitId` to the `AuditTrace` struct:

```rust
pub commit_id: CommitId
```

- [x] **Step 2: Populate audit traces**

In `audit(cell)`, set:

```rust
commit_id: cell.commit_id,
```

- [x] **Step 3: Verify GREEN**

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
- Modify: `docs/superpowers/plans/2026-05-20-commit-aware-audit-traces.md`

- [x] **Step 1: Update docs**

Add commit-aware audit traces to README current scope and Checkout Milestones.

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
git add crates/continuitydb-checkout/src/lib.rs README.md docs/roadmap.md docs/superpowers/plans/2026-05-20-commit-aware-audit-traces.md
git commit -m "feat: add commit-aware audit traces"
```
