# Native Conflict Steward Application Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a native API workflow that audits deterministic conflict-resolution Steward proposals and applies accepted results through the typed dispatcher.

**Architecture:** Introduce a `StewardConflictResolution` result containing the existing conflict audit plus ordered typed application results. Implement `ContinuityDb::resolve_conflicts_with_steward_at` by composing the existing audit method with `apply_accepted_steward_proposal_typed_at`.

**Tech Stack:** Rust 2021, `continuitydb-api` with the `steward` feature, `continuitydb-steward`, `continuitydb-revision`, `continuitydb-memory`.

---

### Task 1: RED Tests

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`

- [x] **Step 1: Add native conflict Steward application test**

Add a feature-gated test named `api_resolves_conflicts_with_steward_records_audit_and_applies_native_link`. The test should create a low-confidence and high-confidence conflicting StateCell pair, call `resolve_conflicts_with_steward_at`, assert one proposal audit record, assert one `StewardApplicationResult::RevisionLink`, assert one native revision-link record, and assert only three StateCells remain visible: two source cells plus one proposal-audit cell.

- [x] **Step 2: Add no-op input test**

Add `api_resolve_conflicts_with_steward_allows_empty_and_singleton_inputs`. It should verify empty and singleton inputs produce no proposals, records, or applications, and do not append proposal audit records.

- [x] **Step 3: Add missing-cell test**

Add `api_resolve_conflicts_with_steward_reports_missing_id_without_mutation`. It should verify missing cell lookup returns `ContinuityError::CellNotFound`, with no proposal audit records and no native revision links.

- [x] **Step 4: Run RED focused tests**

Run:

```bash
cargo test -p continuitydb-api api_resolves_conflicts_with_steward --features steward
cargo test -p continuitydb-api api_resolve_conflicts_with_steward --features steward
```

Expected: FAIL before implementation because `resolve_conflicts_with_steward_at` and `StewardConflictResolution` do not exist.

### Task 2: GREEN Implementation

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`

- [x] **Step 1: Add result type**

Add a feature-gated `StewardConflictResolution` struct near `StewardConflictAudit`:

```rust
#[cfg(feature = "steward")]
pub struct StewardConflictResolution {
    pub audit: StewardConflictAudit,
    pub applications: Vec<StewardApplicationResult>,
}
```

- [x] **Step 2: Add composed API method**

Add `resolve_conflicts_with_steward_at` near `audit_conflict_resolutions_with_steward`. It should call the audit method, iterate `audit.records`, apply each record through `apply_accepted_steward_proposal_typed_at`, collect `Some` results, and return `StewardConflictResolution`.

- [x] **Step 3: Run GREEN focused tests**

Run:

```bash
cargo test -p continuitydb-api api_resolves_conflicts_with_steward --features steward
cargo test -p continuitydb-api api_resolve_conflicts_with_steward --features steward
```

Expected: PASS.

### Task 3: Docs and Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-native-conflict-steward-application.md`

- [x] **Step 1: Update README scope**

Add a current-scope bullet for native conflict-resolution Steward audit-and-application.

- [x] **Step 2: Update roadmap**

Add a Native API milestone for composed conflict-resolution Steward audit/application and a Steward milestone for committing accepted conflict-resolution links as native revision-link records.

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
git add README.md docs/roadmap.md docs/superpowers/specs/2026-05-20-native-conflict-steward-application-design.md docs/superpowers/plans/2026-05-20-native-conflict-steward-application.md crates/continuitydb-api/src/lib.rs
git commit -m "feat: apply conflict steward resolutions"
```
