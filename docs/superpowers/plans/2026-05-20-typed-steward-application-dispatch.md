# Typed Steward Application Dispatch Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an additive typed Steward application dispatcher that can return either committed StateCell IDs or native revision-link records.

**Architecture:** Introduce a feature-gated `StewardApplicationResult` enum in `continuitydb-api`. Add a new `apply_accepted_steward_proposal_typed_at` method that delegates StateCell-producing actions to existing application methods and delegates `LinkRevision` to the native revision-link record path.

**Tech Stack:** Rust 2021, `continuitydb-api`, optional `steward` feature tests.

---

### Task 1: RED API Tests

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`

- [x] **Step 1: Add typed dispatch tests**

Add tests named:

- `api_typed_steward_application_dispatch_returns_state_cell_result`
- `api_typed_steward_application_dispatch_returns_revision_link_record_result`
- `api_typed_steward_application_dispatch_ignores_rejected_record`
- `api_typed_steward_application_dispatch_reports_missing_revision_link_endpoint`

The revision-link test should assert that only source, target, and proposal audit StateCells exist after typed dispatch, while the native revision-link record list contains the returned record.

- [x] **Step 2: Verify RED**

Run:

```bash
cargo test -p continuitydb-api typed_steward_application_dispatch --all-features
```

Expected: FAIL because `StewardApplicationResult` and `apply_accepted_steward_proposal_typed_at` do not exist yet.

### Task 2: GREEN API Implementation

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`

- [x] **Step 1: Add result enum**

Add this feature-gated enum near the other Steward API result structs:

```rust
#[cfg(feature = "steward")]
#[derive(Clone, Debug, PartialEq)]
pub enum StewardApplicationResult {
    StateCell(StateCellId),
    RevisionLink(RevisionLinkRecord),
}
```

- [x] **Step 2: Add typed dispatcher**

Add `apply_accepted_steward_proposal_typed_at` in the `impl<K: StorageKernel> ContinuityDb<K>` block near the existing unified dispatcher. It should:

- return `Ok(None)` for rejected records,
- call `apply_accepted_create_cell_draft_proposal_at`, `apply_accepted_adjust_confidence_proposal_at`, `apply_accepted_label_answerability_proposal_at`, `apply_accepted_mark_frontier_proposal_at`, and `apply_accepted_request_verification_proposal_at` for StateCell-producing actions,
- call `apply_accepted_link_revision_record_proposal_at` for `LinkRevision`,
- map returned values into `StewardApplicationResult`.

- [x] **Step 3: Verify GREEN**

Run:

```bash
cargo test -p continuitydb-api typed_steward_application_dispatch --all-features
```

Expected: PASS.

### Task 3: Docs, Full Gate, Commit

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-typed-steward-application-dispatch.md`

- [x] **Step 1: Update docs**

Record typed accepted Steward application dispatch in README current scope and Native API milestones.

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

Commit with:

```bash
git add README.md docs/roadmap.md crates/continuitydb-api/src/lib.rs docs/superpowers/specs/2026-05-20-typed-steward-application-dispatch-design.md docs/superpowers/plans/2026-05-20-typed-steward-application-dispatch.md
git commit -m "feat: add typed steward application dispatch"
```
