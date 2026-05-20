# Conflict Resolution Policy Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add deterministic conflict resolution recommendations so detected StateCell conflicts can be classified without silently mutating committed truth.

**Architecture:** Keep conflict detection separate from resolution. Add valid-time start access in `continuitydb-core`, then add revision-layer recommendation helpers that classify conflicts as `CandidateSupersession`, `LatestEvidenceWins`, or `NeedsHumanReview`. Recommendations may include proposed revision links, but callers still decide whether to commit them.

**Tech Stack:** Rust 2021, `chrono`, `continuitydb-core`, `continuitydb-revision`, serde.

---

### Task 1: Valid-Time Start Access

**Files:**
- Modify: `crates/continuitydb-core/src/time.rs`
- Modify: `crates/continuitydb-core/src/lib.rs`

- [x] **Step 1: Write failing accessor test**

Add a core test that creates a `ValidTimeRange` and asserts `range.from() == may_20`.

- [x] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p continuitydb-core valid_time_exposes_start
```

Expected: FAIL because `ValidTimeRange::from` does not exist.

- [x] **Step 3: Implement accessor**

Add:

```rust
pub fn from(&self) -> DateTime<Utc> {
    self.from
}
```

### Task 2: Conflict Resolution Recommendations

**Files:**
- Modify: `crates/continuitydb-revision/src/lib.rs`
- Modify: `docs/roadmap.md`
- Create: `docs/superpowers/plans/2026-05-20-conflict-resolution-policy.md`

- [x] **Step 1: Write failing recommendation tests**

Add tests proving:

```rust
let recommendation = recommend_conflict_resolution(&low_confidence, &high_confidence)
    .ok_or_else(|| std::io::Error::other("expected recommendation"))?;
assert_eq!(
    recommendation.kind,
    ConflictResolutionKind::CandidateSupersession
);
assert_eq!(recommendation.winner, Some(high_confidence.id));
assert_eq!(recommendation.loser, Some(low_confidence.id));
assert_eq!(
    recommendation
        .revision
        .targets(high_confidence.id, RevisionLinkKind::Supersedes),
    vec![low_confidence.id]
);
```

And:

```rust
let recommendation = recommend_conflict_resolution(&older, &newer)
    .ok_or_else(|| std::io::Error::other("expected recommendation"))?;
assert_eq!(recommendation.kind, ConflictResolutionKind::LatestEvidenceWins);
assert_eq!(recommendation.winner, Some(newer.id));
assert_eq!(recommendation.loser, Some(older.id));
```

And:

```rust
let recommendation = recommend_conflict_resolution(&left, &right)
    .ok_or_else(|| std::io::Error::other("expected recommendation"))?;
assert_eq!(recommendation.kind, ConflictResolutionKind::NeedsHumanReview);
assert_eq!(recommendation.winner, None);
assert_eq!(recommendation.loser, None);
```

- [x] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p continuitydb-revision conflict_resolution
```

Expected: FAIL because `recommend_conflict_resolution` and `ConflictResolutionKind` do not exist.

- [x] **Step 3: Implement recommendation types and helper**

Add:

```rust
pub enum ConflictResolutionKind {
    CandidateSupersession,
    LatestEvidenceWins,
    NeedsHumanReview,
}

pub struct ConflictResolutionRecommendation {
    pub conflict: CellConflict,
    pub kind: ConflictResolutionKind,
    pub winner: Option<StateCellId>,
    pub loser: Option<StateCellId>,
    pub reason: String,
    pub revision: RevisionGraph,
}
```

Implement `recommend_conflict_resolution(left, right) -> Option<ConflictResolutionRecommendation>`:

- Return `None` if `detect_cell_conflict` returns `None`.
- If max evidence confidence differs by at least `0.20`, recommend `CandidateSupersession` with the higher-confidence cell as winner and add `Supersedes` from winner to loser.
- Else if valid-time starts differ, recommend `LatestEvidenceWins` with later valid-time start as winner and add `Supersedes` from winner to loser.
- Else recommend `NeedsHumanReview` with no winner, no loser, and no supersession links.

- [x] **Step 4: Run focused tests**

Run:

```bash
cargo test -p continuitydb-core valid_time_exposes_start
cargo test -p continuitydb-revision conflict_resolution
cargo test -p continuitydb-revision
```

Expected: PASS.

- [x] **Step 5: Update roadmap**

Add a conflict detection milestone noting deterministic conflict resolution recommendations.

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
git add crates/continuitydb-core/src/time.rs crates/continuitydb-core/src/lib.rs crates/continuitydb-revision/src/lib.rs docs/roadmap.md docs/superpowers/plans/2026-05-20-conflict-resolution-policy.md
git commit -m "feat: recommend conflict resolutions"
```
