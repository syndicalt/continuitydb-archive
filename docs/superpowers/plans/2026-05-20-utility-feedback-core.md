# StateCell Utility Feedback Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the first first-class utility-feedback primitive to `StateCell` so future checkout packing can optimize for learned operational value.

**Architecture:** Keep utility feedback in `continuitydb-core` as bounded scalar signals backed by the existing `Confidence` type. Initialize new cells with neutral feedback and preserve durable compatibility by making the field serde-defaultable.

**Tech Stack:** Rust 2021, serde, existing `Confidence` bounded score type, cargo test/clippy/fmt.

---

### Task 1: Core Utility Feedback Primitive

**Files:**
- Modify: `crates/continuitydb-core/src/lib.rs`
- Modify: `crates/continuitydb-core/src/evidence.rs`
- Modify: `crates/continuitydb-core/src/cell.rs`
- Modify: `docs/roadmap.md`

- [x] **Step 1: Write the failing tests**

Add tests in `crates/continuitydb-core/src/lib.rs`:

```rust
#[test]
fn utility_feedback_score_averages_bounded_signals() -> Result<(), Box<dyn std::error::Error>> {
    let feedback = UtilityFeedback::new(
        Confidence::new(0.9)?,
        Confidence::new(0.6)?,
        Confidence::new(0.3)?,
    );

    assert!((feedback.utility_score() - 0.6).abs() < f32::EPSILON);
    Ok(())
}

#[test]
fn state_cell_starts_with_neutral_utility_feedback() -> Result<(), Box<dyn std::error::Error>> {
    let valid_from = Utc
        .with_ymd_and_hms(2026, 5, 20, 0, 0, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let cell = StateCell::new(
        StateCellId::new(),
        vec![SemanticAnchor::new("project:continuitydb:utility")],
        ValidTimeRange::new(valid_from, None)?,
        Scope::Project("continuitydb".to_string()),
        Answerability::new(vec!["what utility signals apply?".to_string()])?,
        vec![Evidence {
            source: SourceId::new("test"),
            citation: Citation {
                locator: "test://utility".to_string(),
            },
            confidence: Confidence::new(0.8)?,
            trust: vec![TrustSignal::DirectObservation],
        }],
        CellPayload::Text("Utility feedback is tracked.".to_string()),
        CellCost::new(5, 0)?,
    )?;

    assert_eq!(cell.utility_feedback, UtilityFeedback::default());
    assert_eq!(cell.utility_feedback.utility_score(), 0.5);
    Ok(())
}
```

- [x] **Step 2: Run test to verify it fails**

Run: `cargo test -p continuitydb-core utility_feedback`

Expected: FAIL because `UtilityFeedback` and `StateCell::utility_feedback` do not exist yet.

- [x] **Step 3: Implement the minimal core model**

Add `Default` for `Confidence` with neutral `0.5`. Add `UtilityFeedback` to `cell.rs` with `relevance`, `recency`, and `decision_impact` fields, plus `new` and `utility_score`. Add `#[serde(default)] pub utility_feedback: UtilityFeedback` to `StateCell` and initialize it to `UtilityFeedback::default()` in `StateCell::new`. Re-export `UtilityFeedback` from `continuitydb-core`.

- [x] **Step 4: Run focused tests**

Run: `cargo test -p continuitydb-core utility_feedback`

Expected: PASS.

- [x] **Step 5: Update roadmap**

Add a `Utility Feedback Milestones` section to `docs/roadmap.md` with milestone 1 marked implemented.

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
git add crates/continuitydb-core/src/cell.rs crates/continuitydb-core/src/evidence.rs crates/continuitydb-core/src/lib.rs docs/roadmap.md docs/superpowers/plans/2026-05-20-utility-feedback-core.md
git commit -m "feat: add state cell utility feedback"
```
