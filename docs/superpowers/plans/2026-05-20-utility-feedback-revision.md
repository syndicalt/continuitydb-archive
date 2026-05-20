# Utility Feedback Revision Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a deterministic FEEDBACK/LEARN primitive that turns utility feedback into a new append-only `StateCell` version with revision provenance.

**Architecture:** Keep `StateCell` immutable by cloning the previous cell into a successor with a fresh `StateCellId` and updated `UtilityFeedback`. Return the successor plus a `RevisionGraph` containing `Supersedes` and `Predecessor` links back to the prior version, leaving storage append decisions to callers.

**Tech Stack:** Rust 2021, `continuitydb-core`, `continuitydb-revision`, serde-compatible domain types.

---

### Task 1: Utility Feedback Revision Primitive

**Files:**
- Modify: `crates/continuitydb-revision/src/lib.rs`
- Modify: `docs/roadmap.md`
- Create: `docs/superpowers/plans/2026-05-20-utility-feedback-revision.md`

- [x] **Step 1: Write the failing test**

Add a test to `crates/continuitydb-revision/src/lib.rs` that constructs a `StateCell`, calls `revise_utility_feedback`, and asserts:

```rust
let revised = revise_utility_feedback(&previous, feedback);
assert_ne!(revised.cell.id, previous.id);
assert_eq!(revised.cell.utility_feedback, feedback);
assert_eq!(revised.cell.anchors, previous.anchors);
assert_eq!(revised.revision.targets(revised.cell.id, RevisionLinkKind::Supersedes), vec![previous.id]);
assert_eq!(revised.revision.targets(revised.cell.id, RevisionLinkKind::Predecessor), vec![previous.id]);
```

- [x] **Step 2: Run test to verify it fails**

Run: `cargo test -p continuitydb-revision utility_feedback_revision_creates_successor_with_revision_links`

Expected: FAIL because `revise_utility_feedback` does not exist.

- [x] **Step 3: Implement minimal revision helper**

Add:

```rust
pub struct UtilityFeedbackRevision {
    pub cell: StateCell,
    pub revision: RevisionGraph,
}

pub fn revise_utility_feedback(previous: &StateCell, feedback: UtilityFeedback) -> UtilityFeedbackRevision {
    let mut cell = previous.clone();
    cell.id = StateCellId::new();
    cell.utility_feedback = feedback;
    let mut revision = RevisionGraph::default();
    revision.link(cell.id, RevisionLinkKind::Supersedes, previous.id);
    revision.link(cell.id, RevisionLinkKind::Predecessor, previous.id);
    UtilityFeedbackRevision { cell, revision }
}
```

- [x] **Step 4: Run focused tests**

Run:

```bash
cargo test -p continuitydb-revision utility_feedback_revision_creates_successor_with_revision_links
cargo test -p continuitydb-revision
```

Expected: PASS.

- [x] **Step 5: Update roadmap**

Add utility feedback milestone 2 for deterministic append-only feedback revision.

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
git add crates/continuitydb-revision/src/lib.rs docs/roadmap.md docs/superpowers/plans/2026-05-20-utility-feedback-revision.md
git commit -m "feat: revise utility feedback"
```
