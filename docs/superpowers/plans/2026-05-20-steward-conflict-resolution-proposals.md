# Steward Conflict Resolution Proposals Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Expose deterministic conflict-resolution scan output as auditable Steward proposals without committing revision truth.

**Architecture:** Add a `ConflictResolutionSteward` adapter in `continuitydb-steward` that consumes `continuitydb-revision::ConflictResolutionScan`. Recommendations with winner/loser emit `LinkRevision { kind: Supersedes }` proposals; human-review recommendations emit `RequestVerification` proposals. This keeps deterministic revision policy and Steward proposal/audit boundaries separate.

**Tech Stack:** Rust 2021, `chrono`, `continuitydb-core`, `continuitydb-revision`, `continuitydb-steward`.

---

### Task 1: Steward Proposal Bridge

**Files:**
- Create: `crates/continuitydb-steward/src/conflict.rs`
- Modify: `crates/continuitydb-steward/src/lib.rs`
- Modify: `docs/roadmap.md`
- Create: `docs/superpowers/plans/2026-05-20-steward-conflict-resolution-proposals.md`

- [x] **Step 1: Write the failing bridge test**

Create `crates/continuitydb-steward/src/conflict.rs` with a test that:

1. Builds two conflicting cells where one has much higher confidence.
2. Runs `recommend_conflict_resolutions`.
3. Passes the scan into `ConflictResolutionSteward::propose`.
4. Asserts the first proposal is:

```rust
StewardAction::LinkRevision {
    source: high_confidence.id,
    kind: RevisionLinkKind::Supersedes,
    target: low_confidence.id,
}
```

Also assert strict policy accepts the proposal.

- [x] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p continuitydb-steward conflict_resolution_steward
```

Expected: FAIL because `ConflictResolutionSteward` does not exist.

- [x] **Step 3: Implement bridge**

Add:

```rust
pub struct ConflictResolutionSteward {
    identity: StewardIdentity,
}
```

With:

```rust
pub fn new(identity: StewardIdentity) -> Self
pub fn propose(
    &self,
    scan: ConflictResolutionScan,
    created_at: DateTime<Utc>,
) -> Result<Vec<StewardProposal>, StewardError>
```

For each recommendation:

- If both `winner` and `loser` exist, emit a `LinkRevision { source: winner, kind: RevisionLinkKind::Supersedes, target: loser }`.
- Otherwise emit `RequestVerification { cell_id: Some(recommendation.conflict.left), request: "Review unresolved StateCell conflict before accepting a revision.".to_string() }`.
- Use non-empty deterministic rationale and citation strings derived from `recommendation.reason`.

- [x] **Step 4: Export bridge**

Add `mod conflict;` and export `ConflictResolutionSteward` from `crates/continuitydb-steward/src/lib.rs`.

- [x] **Step 5: Run focused tests**

Run:

```bash
cargo test -p continuitydb-steward conflict_resolution_steward
cargo test -p continuitydb-steward
```

Expected: PASS.

- [x] **Step 6: Update roadmap**

Add a Steward milestone noting deterministic conflict-resolution recommendations can now become auditable Steward proposals.

- [x] **Step 7: Run workspace verification**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

Expected: all commands pass.

- [x] **Step 8: Commit**

```bash
git add crates/continuitydb-steward/src/conflict.rs crates/continuitydb-steward/src/lib.rs docs/roadmap.md docs/superpowers/plans/2026-05-20-steward-conflict-resolution-proposals.md
git commit -m "feat: propose conflict resolutions from steward"
```
