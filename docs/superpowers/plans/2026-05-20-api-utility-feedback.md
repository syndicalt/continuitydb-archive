# API Utility Feedback Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add native `continuitydb-api` operations that record utility feedback as append-only StateCell successor revisions.

**Architecture:** Add `continuitydb-revision` as an API crate dependency and use `revise_utility_feedback` inside `ContinuityDb<K>`. The API resolves the prior cell through `CellLookup.cell_id`, creates a successor with updated `UtilityFeedback`, appends it through the backing `StorageKernel`, and returns the successor ID.

**Tech Stack:** Rust 2021, existing workspace crates, `continuitydb-api`, `continuitydb-revision`, TDD.

---

### Task 1: Feedback API Tests

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`
- Modify: `crates/continuitydb-api/Cargo.toml`

- [x] **Step 1: Write failing feedback revision test**

Add a test named `api_records_utility_feedback_as_successor_cell`.

The test should:

```rust
let original_feedback = UtilityFeedback::default();
let feedback = UtilityFeedback::new(
    Confidence::new(0.9)?,
    Confidence::new(0.8)?,
    Confidence::new(0.7)?,
);
let original_id = db.ingest_cell_at(cell, initial_commit)?;
let successor_id = db.record_utility_feedback_at(original_id, feedback, feedback_commit)?;
```

Then assert:

- `successor_id != original_id`
- `db.audit_cell(successor_id)?.cell_id == successor_id`
- the original cell still has `original_feedback`
- the successor has `feedback`
- the successor `system_time.from()` equals `feedback_commit`

- [x] **Step 2: Write failing missing-cell test**

Add `api_record_utility_feedback_reports_missing_id`, asserting:

```rust
matches!(
    db.record_utility_feedback_at(missing_id, feedback, feedback_commit),
    Err(ContinuityError::CellNotFound { cell_id }) if cell_id == missing_id
)
```

- [x] **Step 3: Verify RED**

Run:

```bash
cargo test -p continuitydb-api utility_feedback
```

Expected: FAIL because `record_utility_feedback_at` is not implemented.

### Task 2: Implement Feedback API

**Files:**
- Modify: `crates/continuitydb-api/Cargo.toml`
- Modify: `crates/continuitydb-api/src/lib.rs`

- [x] **Step 1: Add revision dependency**

Add:

```toml
continuitydb-revision = { path = "../continuitydb-revision" }
```

to `crates/continuitydb-api/Cargo.toml`.

- [x] **Step 2: Implement feedback methods**

Add imports:

```rust
use continuitydb_core::{StateCell, StateCellId, UtilityFeedback};
use continuitydb_revision::revise_utility_feedback;
```

Add methods:

```rust
pub fn record_utility_feedback(
    &mut self,
    cell_id: StateCellId,
    feedback: UtilityFeedback,
) -> Result<StateCellId, ContinuityError> {
    self.record_utility_feedback_at(cell_id, feedback, Utc::now())
}

pub fn record_utility_feedback_at(
    &mut self,
    cell_id: StateCellId,
    feedback: UtilityFeedback,
    committed_at: DateTime<Utc>,
) -> Result<StateCellId, ContinuityError> {
    let previous = self.lookup_one_cell(cell_id)?;
    let revision = revise_utility_feedback(&previous, feedback);
    let successor_id = revision.cell.id;
    self.kernel.append_cell_at(revision.cell, committed_at)?;
    Ok(successor_id)
}
```

Use a private helper for ID lookup so `audit_cell` and feedback share missing-cell behavior.

- [x] **Step 3: Verify GREEN**

Run:

```bash
cargo test -p continuitydb-api utility_feedback
cargo test -p continuitydb-api
```

Expected: PASS.

### Task 3: Roadmap and Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-api-utility-feedback.md`

- [x] **Step 1: Update docs**

Add native utility feedback revision to README current scope and Native API Milestones.

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
git add Cargo.lock crates/continuitydb-api README.md docs/roadmap.md docs/superpowers/specs/2026-05-20-api-utility-feedback-design.md docs/superpowers/plans/2026-05-20-api-utility-feedback.md
git commit -m "feat: record utility feedback through api"
```
