# API Conflict Analysis Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add read-only native API operations for deterministic StateCell conflict detection and conflict-resolution recommendations.

**Architecture:** `ContinuityDb<K>` will resolve stored cells by ID using the existing private lookup helper, then delegate to `continuitydb-revision::{detect_cell_conflict, recommend_conflict_resolution}`. Results remain non-mutating and return `Option` for no-conflict cases.

**Tech Stack:** Rust 2021, existing workspace crates, `continuitydb-api`, `continuitydb-revision`, TDD.

---

### Task 1: Conflict API Tests

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`

- [x] **Step 1: Add helper for conflict cells**

Inside the API test module, add a helper that creates a `StateCell` with configurable anchor, payload, valid-from day, and confidence.

- [x] **Step 2: Write failing conflict detection test**

Add `api_detects_conflict_between_stored_cells`.

The test should ingest two cells with the same anchor, overlapping valid time, different payloads, and different confidence values. Call:

```rust
let conflict = db.detect_conflict(left_id, right_id)?;
```

Assert:

- `conflict.is_some()`
- `conflict.unwrap().left == left_id`
- `conflict.kind == CellConflictKind::PayloadMismatch`

- [x] **Step 3: Write failing recommendation test**

Add `api_recommends_conflict_resolution_between_stored_cells`.

Use conflicting cells with confidence `0.95` and `0.60`. Call:

```rust
let recommendation = db.recommend_conflict_resolution(left_id, right_id)?;
```

Assert:

- `recommendation.is_some()`
- `kind == ConflictResolutionKind::CandidateSupersession`
- `winner == Some(left_id)`
- `loser == Some(right_id)`

- [x] **Step 4: Write failing no-conflict and missing-cell tests**

Add:

```rust
api_conflict_analysis_returns_none_for_non_conflicting_cells
api_conflict_analysis_reports_missing_id
```

The no-conflict test should use different semantic anchors and assert `detect_conflict` and `recommend_conflict_resolution` both return `None`.

The missing-cell test should ingest one cell and call both methods with a missing right ID, asserting `ContinuityError::CellNotFound { cell_id }`.

- [x] **Step 5: Verify RED**

Run:

```bash
cargo test -p continuitydb-api conflict
```

Expected: FAIL because `detect_conflict` and `recommend_conflict_resolution` are not implemented on `ContinuityDb`.

### Task 2: Implement Conflict API

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`

- [x] **Step 1: Add imports**

Import the revision functions and types needed by public method signatures:

```rust
use continuitydb_revision::{
    detect_cell_conflict, recommend_conflict_resolution, CellConflict,
    ConflictResolutionRecommendation,
};
```

- [x] **Step 2: Add read-only methods**

Add:

```rust
pub fn detect_conflict(
    &self,
    left_id: StateCellId,
    right_id: StateCellId,
) -> Result<Option<CellConflict>, ContinuityError>

pub fn recommend_conflict_resolution(
    &self,
    left_id: StateCellId,
    right_id: StateCellId,
) -> Result<Option<ConflictResolutionRecommendation>, ContinuityError>
```

Each method should call `lookup_one_cell` for the left cell first, then the right cell, then delegate to the matching revision function.

- [x] **Step 3: Verify GREEN**

Run:

```bash
cargo test -p continuitydb-api conflict
cargo test -p continuitydb-api
```

Expected: PASS.

### Task 3: Roadmap and Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-api-conflict-analysis.md`

- [x] **Step 1: Update docs**

Add read-only conflict analysis to README current scope and Native API Milestones.

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
git add crates/continuitydb-api README.md docs/roadmap.md docs/superpowers/specs/2026-05-20-api-conflict-analysis-design.md docs/superpowers/plans/2026-05-20-api-conflict-analysis.md
git commit -m "feat: expose conflict analysis through api"
```
