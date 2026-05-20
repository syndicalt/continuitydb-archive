# Utility-Aware Checkout Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make deterministic checkout packing use `StateCell` utility feedback so context selection moves toward utility-optimized continuity slices.

**Architecture:** Keep storage lookup constraints unchanged. After lookup and confidence filtering, score candidates inside `continuitydb-checkout` using deterministic evidence confidence plus `StateCell.utility_feedback.utility_score()`, then sort by score before token-budget packing.

**Tech Stack:** Rust 2021, existing `continuitydb-checkout`, `continuitydb-core`, and `continuitydb-memory` crates.

---

### Task 1: Utility-Aware Candidate Ranking

**Files:**
- Modify: `crates/continuitydb-checkout/src/lib.rs`
- Modify: `docs/roadmap.md`
- Create: `docs/superpowers/plans/2026-05-20-utility-aware-checkout.md`

- [x] **Step 1: Write the failing test**

Add this test to `crates/continuitydb-checkout/src/lib.rs`:

```rust
#[test]
fn checkout_prefers_higher_utility_candidate_under_token_budget(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut kernel = MemoryKernel::default();
    let mut high_confidence_low_utility =
        sample_cell("project:continuitydb:confidence-only", 0.95, 10)?;
    high_confidence_low_utility.utility_feedback = UtilityFeedback::new(
        Confidence::new(0.1)?,
        Confidence::new(0.1)?,
        Confidence::new(0.1)?,
    );
    let mut lower_confidence_high_utility =
        sample_cell("project:continuitydb:useful", 0.80, 10)?;
    lower_confidence_high_utility.utility_feedback = UtilityFeedback::new(
        Confidence::new(1.0)?,
        Confidence::new(1.0)?,
        Confidence::new(1.0)?,
    );
    kernel.append_cell(high_confidence_low_utility.clone())?;
    kernel.append_cell(lower_confidence_high_utility.clone())?;

    let slice = checkout(
        &kernel,
        CheckoutRequest {
            scope: Some(Scope::Project("continuitydb".to_string())),
            valid_at: None,
            answerability_question: None,
            evidence_source: None,
            minimum_confidence: Confidence::new(0.7)?,
            token_budget: 10,
        },
    )?;

    assert_eq!(slice.cells, vec![lower_confidence_high_utility]);
    assert_eq!(slice.alternatives.len(), 1);
    assert_eq!(slice.alternatives[0].cell_id, high_confidence_low_utility.id);
    Ok(())
}
```

Also import `UtilityFeedback` in the checkout test module.

- [x] **Step 2: Run test to verify it fails**

Run: `cargo test -p continuitydb-checkout checkout_prefers_higher_utility_candidate_under_token_budget`

Expected: FAIL because checkout currently ranks only by max evidence confidence and selects `project:continuitydb:confidence-only`.

- [x] **Step 3: Implement deterministic utility score**

Replace the confidence-only sort with a helper:

```rust
fn checkout_score(cell: &StateCell) -> f32 {
    (max_confidence(cell) + cell.utility_feedback.utility_score()) / 2.0
}
```

Sort candidates by descending `checkout_score`, then descending `max_confidence`, then ascending first semantic anchor string for deterministic tie-breaking:

```rust
candidates.sort_by(|left, right| {
    checkout_score(right)
        .total_cmp(&checkout_score(left))
        .then_with(|| max_confidence(right).total_cmp(&max_confidence(left)))
        .then_with(|| first_anchor(left).cmp(first_anchor(right)))
});
```

Add:

```rust
fn first_anchor(cell: &StateCell) -> &str {
    cell.anchors
        .first()
        .map(SemanticAnchor::as_str)
        .unwrap_or("")
}
```

Import `SemanticAnchor` in `continuitydb-checkout`.

- [x] **Step 4: Run focused tests**

Run:

```bash
cargo test -p continuitydb-checkout checkout_prefers_higher_utility_candidate_under_token_budget
cargo test -p continuitydb-checkout
```

Expected: PASS.

- [x] **Step 5: Update roadmap**

Add checkout milestone 4 noting deterministic utility-aware candidate ranking.

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
git add crates/continuitydb-checkout/src/lib.rs docs/roadmap.md docs/superpowers/plans/2026-05-20-utility-aware-checkout.md
git commit -m "feat: rank checkout by utility"
```
