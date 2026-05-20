# Anchor-Scoped Checkout Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add semantic-anchor constraints to checkout, typed queries, and strict text `CHECKOUT` queries.

**Architecture:** `CheckoutRequest` gains `semantic_anchor: Option<SemanticAnchor>` and pushes it into `CellLookup.semantic_anchor`. `QueryRequirements` gains the same optional field and compiles it into checkout requests. The text parser accepts `semantic_anchor = "..."` and stores a `SemanticAnchor`.

**Tech Stack:** Rust 2021, `continuitydb-core::SemanticAnchor`, `continuitydb-checkout`, `continuitydb-query`, existing storage semantic-anchor lookup.

---

## File Structure

- Modify `crates/continuitydb-checkout/src/lib.rs`: add request field, pushdown, direct constructors, and tests.
- Modify `crates/continuitydb-query/src/lib.rs`: add typed requirements field and tests.
- Modify `crates/continuitydb-query/src/text.rs`: parse semantic-anchor constraint.
- Modify `crates/continuitydb-api/src/lib.rs` and `crates/continuitydb-cli/src/main.rs`: preserve direct checkout construction behavior with `semantic_anchor: None`.
- Modify `README.md`: record anchor-scoped checkout/query constraints.
- Modify `docs/roadmap.md`: add Checkout and Query Language milestones.
- Modify this plan: mark steps complete as executed.

## Task 1: Failing Checkout Tests

**Files:**
- Modify: `crates/continuitydb-checkout/src/lib.rs`

- [x] **Step 1: Add semantic anchor field to existing request literals in checkout tests**

For every `CheckoutRequest { ... }` literal in `crates/continuitydb-checkout/src/lib.rs`, add:

```rust
semantic_anchor: None,
```

immediately before `scope`.

- [x] **Step 2: Add semantic anchor pushdown test**

Add this test after `checkout_pushes_semantic_constraints_to_kernel`:

```rust
#[test]
fn checkout_pushes_semantic_anchor_to_kernel() -> Result<(), Box<dyn std::error::Error>> {
    let kernel = RecordingKernel::default();
    checkout(
        &kernel,
        CheckoutRequest {
            semantic_anchor: Some(SemanticAnchor::new("project:continuitydb:release-status")),
            scope: None,
            valid_at: None,
            system_at: None,
            commit_id: None,
            activation: None,
            answerability_question: None,
            evidence_source: None,
            dependency_target: None,
            dependency_kind: None,
            minimum_confidence: Confidence::new(0.8)?,
            token_budget: 10,
        },
    )?;

    let lookup = kernel
        .lookup
        .borrow()
        .clone()
        .ok_or_else(|| std::io::Error::other("lookup was not captured"))?;
    assert_eq!(
        lookup.semantic_anchor.as_deref(),
        Some("project:continuitydb:release-status")
    );
    Ok(())
}
```

- [x] **Step 3: Add semantic anchor materialization test**

Add this test near other checkout materialization tests:

```rust
#[test]
fn checkout_filters_by_semantic_anchor() -> Result<(), Box<dyn std::error::Error>> {
    let mut kernel = MemoryKernel::default();
    let selected = sample_cell("project:continuitydb:anchor-selected", 0.91, 10)?;
    let unrelated = sample_cell("project:continuitydb:anchor-unrelated", 0.9, 10)?;
    let selected = append_committed(&mut kernel, selected)?;
    append_committed(&mut kernel, unrelated)?;

    let slice = checkout(
        &kernel,
        CheckoutRequest {
            semantic_anchor: Some(SemanticAnchor::new("project:continuitydb:anchor-selected")),
            scope: None,
            valid_at: None,
            system_at: None,
            commit_id: None,
            activation: None,
            answerability_question: None,
            evidence_source: None,
            dependency_target: None,
            dependency_kind: None,
            minimum_confidence: Confidence::new(0.7)?,
            token_budget: 20,
        },
    )?;

    assert_eq!(slice.cells, vec![selected]);
    Ok(())
}
```

- [x] **Step 4: Verify RED**

Run:

```bash
cargo test -p continuitydb-checkout semantic_anchor --all-features
```

Expected: FAIL because `CheckoutRequest` has no `semantic_anchor` field.

## Task 2: Checkout Implementation

**Files:**
- Modify: `crates/continuitydb-checkout/src/lib.rs`

- [x] **Step 1: Add request field**

Add this field before `scope`:

```rust
/// Optional semantic anchor filter.
pub semantic_anchor: Option<SemanticAnchor>,
```

- [x] **Step 2: Push semantic anchor into storage lookup**

Replace `semantic_anchor: None` in the `CellLookup` construction with:

```rust
semantic_anchor: request
    .semantic_anchor
    .as_ref()
    .map(|anchor| anchor.as_str().to_string()),
```

- [x] **Step 3: Verify GREEN**

Run:

```bash
cargo test -p continuitydb-checkout semantic_anchor --all-features
cargo test -p continuitydb-checkout --all-features
```

Expected: PASS.

- [x] **Step 4: Commit checkout implementation**

Consolidated into the final anchor-scoped checkout commit after dependent query/API/CLI constructors were updated, so the repository did not receive a known broken intermediate commit.

```bash
git add crates/continuitydb-checkout/src/lib.rs docs/superpowers/plans/2026-05-20-anchor-scoped-checkout.md
git commit -m "feat: add anchor-scoped checkout"
```

## Task 3: Failing Query Tests

**Files:**
- Modify: `crates/continuitydb-query/src/lib.rs`

- [x] **Step 1: Add SemanticAnchor imports**

Update production and test core imports to include `SemanticAnchor`.

- [x] **Step 2: Add default assertion**

In `minimal_checkout_query_compiles_task_answerability_and_defaults`, assert:

```rust
assert_eq!(request.semantic_anchor, None);
```

- [x] **Step 3: Add typed query semantic anchor compile test**

Add this test after the activation compile test:

```rust
#[test]
fn checkout_query_compiles_semantic_anchor_requirement(
) -> Result<(), Box<dyn std::error::Error>> {
    let anchor = SemanticAnchor::new("project:continuitydb:release-status");
    let task = QueryTask::new("release-status", "what is the release status?");
    let requirements = QueryRequirements {
        semantic_anchor: Some(anchor.clone()),
        ..QueryRequirements::default()
    };

    let request = CheckoutQuery::new(task)
        .with_requirements(requirements)
        .compile_checkout()?;

    assert_eq!(request.semantic_anchor, Some(anchor));
    Ok(())
}
```

- [x] **Step 4: Add text parser semantic anchor test**

Add this test after `text_query_parses_activation_constraint`:

```rust
#[test]
fn text_query_parses_semantic_anchor_constraint() -> Result<(), Box<dyn std::error::Error>> {
    let query = parse_query_text(
        r#"CHECKOUT "release-status" ANSWER "what is the release status?"
WHERE semantic_anchor = "project:continuitydb:release-status""#,
    )?;

    let ContinuityQuery::Checkout(checkout) = query;
    assert_eq!(
        checkout.requirements().semantic_anchor,
        Some(SemanticAnchor::new("project:continuitydb:release-status"))
    );
    Ok(())
}
```

- [x] **Step 5: Verify RED**

Run:

```bash
cargo test -p continuitydb-query semantic_anchor --all-features
```

Expected: FAIL because `QueryRequirements` and the text parser do not support semantic anchors yet.

## Task 4: Query Implementation

**Files:**
- Modify: `crates/continuitydb-query/src/lib.rs`
- Modify: `crates/continuitydb-query/src/text.rs`

- [x] **Step 1: Add QueryRequirements field and default**

Add this field before `scope`:

```rust
/// Optional semantic anchor filter.
pub semantic_anchor: Option<SemanticAnchor>,
```

Set it to `None` in `Default`.

- [x] **Step 2: Compile semantic anchor**

Add this field in `CheckoutQuery::compile_checkout`:

```rust
semantic_anchor: self.requirements.semantic_anchor,
```

- [x] **Step 3: Parse text semantic anchor**

Import `SemanticAnchor` in `text.rs`, then add this branch before `scope`:

```rust
"semantic_anchor" => {
    self.expect_token(Token::Eq)?;
    requirements.semantic_anchor = Some(SemanticAnchor::new(self.expect_string()?));
}
```

- [x] **Step 4: Verify GREEN**

Run:

```bash
cargo test -p continuitydb-query semantic_anchor --all-features
cargo test -p continuitydb-query --all-features
```

Expected: PASS.

- [x] **Step 5: Commit query implementation**

Consolidated into the final anchor-scoped checkout commit after API/CLI direct constructors and documentation were updated, so committed code remained workspace-green.

```bash
git add crates/continuitydb-query/src/lib.rs crates/continuitydb-query/src/text.rs docs/superpowers/plans/2026-05-20-anchor-scoped-checkout.md
git commit -m "feat: add anchor query constraints"
```

## Task 5: Direct Constructors and Documentation

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`
- Modify: `crates/continuitydb-cli/src/main.rs`
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-anchor-scoped-checkout.md`

- [x] **Step 1: Update direct checkout constructors**

Add `semantic_anchor: None` to direct `CheckoutRequest` literals in `continuitydb-api` and `continuitydb-cli`.

- [x] **Step 2: Update README current scope**

Add this bullet after deterministic checkout:

```markdown
- Semantic-anchor scoped checkout and strict text query constraints.
```

- [x] **Step 3: Update Checkout roadmap**

Add this milestone after activation-aware checkout and renumber later checkout milestones:

```markdown
7. Add semantic-anchor checkout constraints. Implemented `CheckoutRequest.semantic_anchor` with pushdown into `CellLookup.semantic_anchor` so callers can materialize StateCells by stable semantic identity through deterministic checkout.
```

- [x] **Step 4: Update Query Language roadmap**

Add this milestone after activation constraints:

```markdown
11. Add semantic-anchor constraints to typed and text checkout queries. Implemented `QueryRequirements.semantic_anchor` and strict text `semantic_anchor = "..."` parsing so query files can materialize anchor-scoped continuity slices.
```

- [x] **Step 5: Run full verification gate**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

Expected: all commands exit successfully.

- [x] **Step 6: Commit final updates**

```bash
git add crates/continuitydb-api/src/lib.rs crates/continuitydb-cli/src/main.rs README.md docs/roadmap.md docs/superpowers/plans/2026-05-20-anchor-scoped-checkout.md
git commit -m "docs: record anchor-scoped checkout"
```

## Self-Review

- Spec coverage: Tasks add checkout pushdown, materialization, typed query compilation, text parsing, direct constructor updates, docs, and verification.
- Placeholder scan: No placeholders or vague implementation steps remain.
- Type consistency: `SemanticAnchor`, `CheckoutRequest.semantic_anchor`, `QueryRequirements.semantic_anchor`, and `CellLookup.semantic_anchor` match existing crate boundaries.
