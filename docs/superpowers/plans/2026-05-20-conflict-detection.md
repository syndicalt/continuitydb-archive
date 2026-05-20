# Conflict Detection Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the first deterministic conflict-detection primitive for StateCells with overlapping valid time, shared semantic anchor, and incompatible payload.

**Architecture:** Add `ValidTimeRange::overlaps` in `continuitydb-core` so revision logic can reason about real-world validity intervals without exposing private fields. Add `detect_cell_conflict` in `continuitydb-revision` that returns structured conflict metadata and a `RevisionGraph` with reciprocal `ConflictsWith` links.

**Tech Stack:** Rust 2021, `chrono`, `continuitydb-core`, `continuitydb-revision`, serde.

---

### Task 1: Valid Time Overlap

**Files:**
- Modify: `crates/continuitydb-core/src/time.rs`
- Modify: `crates/continuitydb-core/src/lib.rs`

- [x] **Step 1: Write failing overlap test**

Add a core test that creates two overlapping half-open valid-time ranges and one adjacent non-overlapping range, then asserts `overlaps` is true only for the overlapping range.

- [x] **Step 2: Run test to verify it fails**

Run: `cargo test -p continuitydb-core valid_time_overlap`

Expected: FAIL because `ValidTimeRange::overlaps` does not exist.

- [x] **Step 3: Implement overlap**

Add:

```rust
pub fn overlaps(&self, other: &Self) -> bool {
    let self_starts_before_other_ends = other.to.map_or(true, |other_to| self.from < other_to);
    let other_starts_before_self_ends = self.to.map_or(true, |self_to| other.from < self_to);
    self_starts_before_other_ends && other_starts_before_self_ends
}
```

### Task 2: Revision Conflict Detection

**Files:**
- Modify: `crates/continuitydb-revision/src/lib.rs`
- Modify: `docs/roadmap.md`
- Create: `docs/superpowers/plans/2026-05-20-conflict-detection.md`

- [x] **Step 1: Write failing conflict tests**

Add one test proving `detect_cell_conflict` identifies two cells with the same anchor, overlapping valid time, and different payloads. Add one test proving same anchor but adjacent valid-time ranges do not conflict.

- [x] **Step 2: Run test to verify it fails**

Run: `cargo test -p continuitydb-revision conflict_detection`

Expected: FAIL because `detect_cell_conflict` does not exist.

- [x] **Step 3: Implement conflict detection**

Add `CellConflictKind::PayloadMismatch`, `CellConflict`, and `detect_cell_conflict(left, right) -> Option<CellConflict>`. The detection rule is: shared semantic anchor, overlapping valid time, and different payload.

- [x] **Step 4: Run focused tests**

Run:

```bash
cargo test -p continuitydb-core valid_time_overlap
cargo test -p continuitydb-revision conflict_detection
cargo test -p continuitydb-revision
```

Expected: PASS.

- [x] **Step 5: Update roadmap**

Add a conflict/revision milestone for deterministic same-anchor payload conflict detection.

- [x] **Step 6: Run workspace verification**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

Expected: all commands pass.

- [x] **Step 7: Commit**

```bash
git add crates/continuitydb-core/src/time.rs crates/continuitydb-core/src/lib.rs crates/continuitydb-revision/src/lib.rs docs/roadmap.md docs/superpowers/plans/2026-05-20-conflict-detection.md
git commit -m "feat: detect state cell conflicts"
```
