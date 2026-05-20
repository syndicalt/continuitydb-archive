# Conflict Resolution Scan Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a deterministic batch conflict-resolution scan that recommends policy outcomes for every detected StateCell conflict in a candidate set.

**Architecture:** Reuse existing pairwise `scan_cell_conflicts` and `recommend_conflict_resolution` semantics, but expose a single batch API over cells so checkout and steward layers can ask for all conflict recommendations at once. The batch result carries recommendations in deterministic input-pair order plus aggregate conflict and proposed supersession revision graphs.

**Tech Stack:** Rust 2021, `continuitydb-core`, `continuitydb-revision`, serde.

---

### Task 1: Batch Conflict Resolution Recommendations

**Files:**
- Modify: `crates/continuitydb-revision/src/lib.rs`
- Modify: `docs/roadmap.md`
- Create: `docs/superpowers/plans/2026-05-20-conflict-resolution-scan.md`

- [x] **Step 1: Write the failing batch recommendation test**

Add a test to `crates/continuitydb-revision/src/lib.rs` that builds:

```rust
let low_confidence = sample_cell_with_anchor_payload_time_and_confidence(
    "project:continuitydb:release-status",
    "Release is blocked.",
    20,
    Some(23),
    0.55,
)?;
let high_confidence = sample_cell_with_anchor_payload_time_and_confidence(
    "project:continuitydb:release-status",
    "Release is green.",
    21,
    Some(23),
    0.9,
)?;
let older = sample_cell_with_anchor_payload_time_and_confidence(
    "project:continuitydb:roadmap-status",
    "Roadmap is stale.",
    20,
    Some(23),
    0.8,
)?;
let newer = sample_cell_with_anchor_payload_time_and_confidence(
    "project:continuitydb:roadmap-status",
    "Roadmap is current.",
    21,
    Some(23),
    0.79,
)?;
let unrelated = sample_cell_with_anchor_payload_time_and_confidence(
    "project:continuitydb:storage",
    "Storage milestone is separate.",
    20,
    Some(23),
    0.95,
)?;
```

Call:

```rust
let scan = recommend_conflict_resolutions(&[
    low_confidence.clone(),
    high_confidence.clone(),
    older.clone(),
    newer.clone(),
    unrelated,
]);
```

Assert:

```rust
assert_eq!(scan.recommendations.len(), 2);
assert_eq!(
    scan.recommendations[0].kind,
    ConflictResolutionKind::CandidateSupersession
);
assert_eq!(scan.recommendations[0].winner, Some(high_confidence.id));
assert_eq!(
    scan.recommendations[1].kind,
    ConflictResolutionKind::LatestEvidenceWins
);
assert_eq!(scan.recommendations[1].winner, Some(newer.id));
assert_eq!(
    scan.conflicts.targets(low_confidence.id, RevisionLinkKind::ConflictsWith),
    vec![high_confidence.id]
);
assert_eq!(
    scan.proposed_revisions
        .targets(high_confidence.id, RevisionLinkKind::Supersedes),
    vec![low_confidence.id]
);
assert_eq!(
    scan.proposed_revisions
        .targets(newer.id, RevisionLinkKind::Supersedes),
    vec![older.id]
);
```

- [x] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p continuitydb-revision conflict_resolution_scan
```

Expected: FAIL because `recommend_conflict_resolutions` does not exist.

- [x] **Step 3: Implement batch result and scanner**

Add:

```rust
pub struct ConflictResolutionScan {
    pub recommendations: Vec<ConflictResolutionRecommendation>,
    pub conflicts: RevisionGraph,
    pub proposed_revisions: RevisionGraph,
}
```

Implement `recommend_conflict_resolutions(cells: &[StateCell]) -> ConflictResolutionScan` by walking unordered pairs in deterministic input order, calling `recommend_conflict_resolution(left, right)`, appending recommendations, recording reciprocal `ConflictsWith` links in `conflicts`, and copying each recommendation's `Supersedes` winner/loser link into `proposed_revisions` when both are present.

- [x] **Step 4: Run focused tests**

Run:

```bash
cargo test -p continuitydb-revision conflict_resolution_scan
cargo test -p continuitydb-revision
```

Expected: PASS.

- [x] **Step 5: Update roadmap**

Add a conflict detection milestone noting deterministic batch conflict resolution recommendations.

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
git add crates/continuitydb-revision/src/lib.rs docs/roadmap.md docs/superpowers/plans/2026-05-20-conflict-resolution-scan.md
git commit -m "feat: scan conflict resolutions"
```
