# Conflict Scan Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a deterministic conflict scan primitive that finds every pairwise StateCell conflict in a candidate set and returns aggregate conflict revision links.

**Architecture:** Keep single-pair detection in `detect_cell_conflict` and layer `scan_cell_conflicts` over it. The scan walks unordered pairs in input order, records each detected `CellConflict`, and builds one aggregate `RevisionGraph` with reciprocal `ConflictsWith` links for every conflict.

**Tech Stack:** Rust 2021, `continuitydb-core`, `continuitydb-revision`, serde.

---

### Task 1: Pairwise Conflict Scan

**Files:**
- Modify: `crates/continuitydb-revision/src/lib.rs`
- Modify: `docs/roadmap.md`
- Create: `docs/superpowers/plans/2026-05-20-conflict-scan.md`

- [x] **Step 1: Write the failing scan test**

Add a test to `crates/continuitydb-revision/src/lib.rs` that builds four cells: two conflicting release-status cells, one adjacent release-status cell, and one unrelated roadmap cell. Call:

```rust
let scan = scan_cell_conflicts(&[left.clone(), right.clone(), adjacent, unrelated]);
```

Assert:

```rust
assert_eq!(scan.conflicts.len(), 1);
assert_eq!(scan.conflicts[0].left, left.id);
assert_eq!(scan.conflicts[0].right, right.id);
assert_eq!(
    scan.revision.targets(left.id, RevisionLinkKind::ConflictsWith),
    vec![right.id]
);
assert_eq!(
    scan.revision.targets(right.id, RevisionLinkKind::ConflictsWith),
    vec![left.id]
);
```

- [x] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p continuitydb-revision conflict_scan
```

Expected: FAIL because `scan_cell_conflicts` does not exist.

- [x] **Step 3: Implement scan result and scanner**

Add:

```rust
/// Deterministic conflict scan over a candidate StateCell set.
pub struct CellConflictScan {
    /// Pairwise conflicts found in deterministic input-pair order.
    pub conflicts: Vec<CellConflict>,
    /// Aggregate revision links for all detected conflicts.
    pub revision: RevisionGraph,
}

/// Finds all deterministic conflicts across unordered pairs in input order.
pub fn scan_cell_conflicts(cells: &[StateCell]) -> CellConflictScan {
    let mut conflicts = Vec::new();
    let mut revision = RevisionGraph::default();

    for (left_index, left) in cells.iter().enumerate() {
        for right in cells.iter().skip(left_index + 1) {
            if let Some(conflict) = detect_cell_conflict(left, right) {
                revision.link(conflict.left, RevisionLinkKind::ConflictsWith, conflict.right);
                revision.link(conflict.right, RevisionLinkKind::ConflictsWith, conflict.left);
                conflicts.push(conflict);
            }
        }
    }

    CellConflictScan { conflicts, revision }
}
```

- [x] **Step 4: Run focused tests**

Run:

```bash
cargo test -p continuitydb-revision conflict_scan
cargo test -p continuitydb-revision
```

Expected: PASS.

- [x] **Step 5: Update roadmap**

Add a second conflict detection milestone noting deterministic candidate-set conflict scanning and aggregate `ConflictsWith` links.

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
git add crates/continuitydb-revision/src/lib.rs docs/roadmap.md docs/superpowers/plans/2026-05-20-conflict-scan.md
git commit -m "feat: scan state cell conflicts"
```
