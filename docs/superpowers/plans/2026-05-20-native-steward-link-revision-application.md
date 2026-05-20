# Native Steward LinkRevision Application Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a native API method that applies accepted Steward `LinkRevision` proposals by appending an auditable operational revision-link StateCell.

**Architecture:** The API keeps model output proposal-only. Accepted revision links are materialized as new StateCells with derived evidence, full proposal audit payload, and endpoint dependencies until the storage kernel grows a dedicated revision-link record type.

**Tech Stack:** Rust, `continuitydb-api`, `continuitydb-core`, `continuitydb-steward`, in-memory kernel tests.

---

### Task 1: RED API Tests

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`

- [x] **Step 1: Add failing tests**

Add tests covering accepted, rejected, unsupported, missing-source, and missing-target `LinkRevision` application.

- [x] **Step 2: Verify RED**

Run: `cargo test -p continuitydb-api --features steward link_revision_application`

Expected: FAIL because `apply_accepted_link_revision_proposal_at` does not exist.

### Task 2: Native API Implementation

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`

- [x] **Step 1: Implement the API method**

Add `ContinuityDb::apply_accepted_link_revision_proposal_at`.

- [x] **Step 2: Verify GREEN**

Run: `cargo test -p continuitydb-api --features steward link_revision_application`

Expected: PASS.

### Task 3: Docs, Gate, Commit

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-native-steward-link-revision-application.md`

- [x] **Step 1: Update docs**

Record the new native API and Steward milestones.

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

Commit with `feat: apply accepted revision link steward proposals`.
