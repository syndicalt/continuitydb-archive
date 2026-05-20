# API Batch Conflict Analysis Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add native API operations for deterministic batch StateCell conflict scans and batch conflict-resolution recommendations.

**Architecture:** `ContinuityDb<K>` resolves input `StateCellId` values into stored `StateCell` values in caller order, then delegates to `continuitydb-revision::scan_cell_conflicts` or `recommend_conflict_resolutions`. The API remains read-only and returns existing scan structs.

**Tech Stack:** Rust 2021, existing workspace crates, `continuitydb-api`, `continuitydb-revision`, TDD.

---

### Task 1: Batch Conflict API Tests

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`

- [x] **Step 1: Write failing batch conflict scan test**

Add `api_detects_conflicts_for_stored_cell_set`.

The test should ingest:

- `left`: anchor `project:continuitydb:batch-conflict`, payload `release is ready`, confidence `0.95`
- `right`: same anchor, payload `release is blocked`, confidence `0.60`
- `unrelated`: different anchor, any payload

Call:

```rust
let scan = db.detect_conflicts([left_id, right_id, unrelated_id])?;
```

Assert `scan.conflicts.len() == 1`, and the first conflict has `left == left_id` and `right == right_id`.

- [x] **Step 2: Write failing batch recommendation test**

Add `api_recommends_conflict_resolutions_for_stored_cell_set`.

Use the same three cells. Call:

```rust
let scan = db.recommend_conflict_resolutions([left_id, right_id, unrelated_id])?;
```

Assert `scan.recommendations.len() == 1`, `kind == CandidateSupersession`, `winner == Some(left_id)`, and `loser == Some(right_id)`.

- [x] **Step 3: Write failing empty/singleton and missing tests**

Add:

```rust
api_batch_conflict_analysis_allows_empty_and_singleton_inputs
api_batch_conflict_analysis_reports_missing_id
```

Empty/singleton should assert both scans have no conflicts/recommendations. Missing should pass one stored ID and one missing ID, then assert `CellNotFound` for both batch methods.

- [x] **Step 4: Verify RED**

Run:

```bash
cargo test -p continuitydb-api batch_conflict
```

Expected: FAIL because `detect_conflicts` and `recommend_conflict_resolutions` are not implemented on `ContinuityDb`.

### Task 2: Implement Batch Conflict API

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`

- [x] **Step 1: Add imports**

Import:

```rust
use continuitydb_revision::{
    recommend_conflict_resolutions, scan_cell_conflicts, CellConflictScan,
    ConflictResolutionScan,
};
```

- [x] **Step 2: Add batch methods**

Add public methods:

```rust
pub fn detect_conflicts<I>(&self, cell_ids: I) -> Result<CellConflictScan, ContinuityError>
where
    I: IntoIterator<Item = StateCellId>

pub fn recommend_conflict_resolutions<I>(
    &self,
    cell_ids: I,
) -> Result<ConflictResolutionScan, ContinuityError>
where
    I: IntoIterator<Item = StateCellId>
```

Add a private helper:

```rust
fn lookup_cells_in_order<I>(&self, cell_ids: I) -> Result<Vec<StateCell>, ContinuityError>
where
    I: IntoIterator<Item = StateCellId>
```

The helper should call `lookup_one_cell` for each ID and preserve input order.

- [x] **Step 3: Verify GREEN**

Run:

```bash
cargo test -p continuitydb-api batch_conflict
cargo test -p continuitydb-api
```

Expected: PASS.

### Task 3: Roadmap and Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-api-batch-conflict-analysis.md`

- [x] **Step 1: Update docs**

Add batch conflict analysis to README current scope and Native API Milestones.

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
git add crates/continuitydb-api README.md docs/roadmap.md docs/superpowers/specs/2026-05-20-api-batch-conflict-analysis-design.md docs/superpowers/plans/2026-05-20-api-batch-conflict-analysis.md
git commit -m "feat: expose batch conflict analysis through api"
```
