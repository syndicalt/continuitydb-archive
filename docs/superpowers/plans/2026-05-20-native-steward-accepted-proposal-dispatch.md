# Native Steward Accepted Proposal Dispatch Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a unified native API method that applies any accepted Steward proposal action by dispatching to the existing deterministic application methods.

**Architecture:** The dispatcher performs no new mutation logic. It matches the proposal action and delegates to the action-specific application method, preserving existing validation, error, and append-only semantics.

**Tech Stack:** Rust, `continuitydb-api`, `continuitydb-steward`, in-memory kernel tests.

---

### Task 1: RED API Tests

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`

- [x] **Step 1: Add failing tests**

Add tests covering accepted create-cell dispatch, accepted frontier dispatch, rejected dispatch, and missing-cell error propagation.

- [x] **Step 2: Verify RED**

Run: `cargo test -p continuitydb-api --features steward accepted_steward_proposal_dispatch`

Expected: FAIL because `apply_accepted_steward_proposal_at` does not exist.

### Task 2: Native API Dispatcher

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`

- [x] **Step 1: Implement the dispatcher**

Add `ContinuityDb::apply_accepted_steward_proposal_at`.

- [x] **Step 2: Verify GREEN**

Run: `cargo test -p continuitydb-api --features steward accepted_steward_proposal_dispatch`

Expected: PASS.

### Task 3: Docs, Gate, Commit

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-native-steward-accepted-proposal-dispatch.md`

- [x] **Step 1: Update docs**

Record the new native API and Steward milestone.

- [x] **Step 2: Run full verification**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

- [x] **Step 3: Commit**

Commit with `feat: dispatch accepted steward proposal application`.
