# Native Frontier Steward Application Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a native API workflow that audits subscribed frontier/watch Steward proposals and applies accepted results through the typed dispatcher.

**Architecture:** Introduce `StewardFrontierResolution`, then implement `ContinuityDb::resolve_frontier_watch_with_steward_at` by composing the existing frontier audit method with `apply_accepted_steward_proposal_typed_at`.

**Tech Stack:** Rust 2021, `continuitydb-api` with the `steward` feature, `continuitydb-steward`, `continuitydb-memory`.

---

### Task 1: RED Tests

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`

- [x] **Step 1: Add stale-evidence application test**

Add `api_resolves_frontier_watch_with_steward_records_audit_and_applies_verification`. It should ingest a target StateCell, run a subscribed `StaleEvidence` event through `resolve_frontier_watch_with_steward_at`, assert one audit record, one `StateCell` application result, one proposal-audit StateCell, and one verification-work StateCell.

- [x] **Step 2: Add high-impact frontier application test**

Add `api_resolves_frontier_watch_with_steward_applies_mark_frontier`. It should ingest a target StateCell, run a subscribed `HighImpactUncertainty` event, assert one `StateCell` application result, and verify the resulting successor cell has `ActivationState::Frontier`.

- [x] **Step 3: Add no-op event test**

Add `api_resolve_frontier_watch_with_steward_ignores_unsubscribed_and_benign_events`. It should verify no proposals, records, or applications are produced for an unsubscribed cell and a benign event.

- [x] **Step 4: Run RED focused tests**

Run:

```bash
cargo test -p continuitydb-api api_resolves_frontier_watch_with_steward --features steward
cargo test -p continuitydb-api api_resolve_frontier_watch_with_steward --features steward
```

Expected: FAIL before implementation because `resolve_frontier_watch_with_steward_at` and `StewardFrontierResolution` do not exist.

### Task 2: GREEN Implementation

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`

- [x] **Step 1: Add result type**

Add a feature-gated `StewardFrontierResolution` struct near `StewardFrontierAudit`:

```rust
#[cfg(feature = "steward")]
pub struct StewardFrontierResolution {
    pub audit: StewardFrontierAudit,
    pub applications: Vec<StewardApplicationResult>,
}
```

- [x] **Step 2: Add composed API method**

Add `resolve_frontier_watch_with_steward_at` near `audit_frontier_watch_with_steward`. It should call the audit method, iterate `audit.records`, apply each record through `apply_accepted_steward_proposal_typed_at`, collect `Some` results, and return `StewardFrontierResolution`.

- [x] **Step 3: Run GREEN focused tests**

Run:

```bash
cargo test -p continuitydb-api api_resolves_frontier_watch_with_steward --features steward
cargo test -p continuitydb-api api_resolve_frontier_watch_with_steward --features steward
```

Expected: PASS.

### Task 3: Docs and Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-native-frontier-steward-application.md`

- [x] **Step 1: Update README scope**

Add a current-scope bullet for native frontier/watch Steward audit-and-application.

- [x] **Step 2: Update roadmap**

Add a Native API milestone for composed frontier/watch Steward audit/application and a Steward milestone for committing accepted frontier/watch maintenance results through one API call.

- [x] **Step 3: Run full verification gate**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

Expected: PASS.

- [x] **Step 4: Mark plan complete**

Check off all completed boxes in this plan.

- [x] **Step 5: Commit**

Commit with:

```bash
git add README.md docs/roadmap.md docs/superpowers/specs/2026-05-20-native-frontier-steward-application-design.md docs/superpowers/plans/2026-05-20-native-frontier-steward-application.md crates/continuitydb-api/src/lib.rs
git commit -m "feat: apply frontier steward resolutions"
```
