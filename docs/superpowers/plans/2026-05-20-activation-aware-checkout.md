# Activation-Aware Checkout Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add activation-state constraints to checkout, typed queries, and strict text `CHECKOUT` queries.

**Architecture:** `CheckoutRequest` gains `activation: Option<ActivationState>` and pushes it into `CellLookup`. `QueryRequirements` gains the same optional field and compiles it into checkout requests. The text parser accepts `activation = dormant|active|frontier|retired` and stores the parsed `ActivationState`.

**Tech Stack:** Rust 2021, `continuitydb-core::ActivationState`, `continuitydb-checkout`, `continuitydb-query`, existing storage lookup activation indexes.

---

## File Structure

- Modify `crates/continuitydb-checkout/src/lib.rs`: add checkout request field, pushdown, and tests.
- Modify `crates/continuitydb-query/src/lib.rs`: add typed requirements field and tests.
- Modify `crates/continuitydb-query/src/text.rs`: parse activation constraint.
- Modify `README.md`: record activation-aware checkout/query constraints.
- Modify `docs/roadmap.md`: add Checkout and Query Language milestones.
- Modify this plan: mark steps complete as executed.

## Task 1: Failing Checkout Tests

**Files:**
- Modify: `crates/continuitydb-checkout/src/lib.rs`

- [x] **Step 1: Add activation field to existing request literals in tests**

For every `CheckoutRequest { ... }` literal in this file, add:

```rust
activation: None,
```

right after `commit_id`. This keeps existing tests focused on current behavior before the new activation tests are added.

- [x] **Step 2: Add activation pushdown test**

Add this test after `checkout_pushes_dependency_constraints_to_kernel`:

```rust
#[test]
fn checkout_pushes_activation_to_kernel() -> Result<(), Box<dyn std::error::Error>> {
    let kernel = RecordingKernel::default();
    checkout(
        &kernel,
        CheckoutRequest {
            scope: None,
            valid_at: None,
            system_at: None,
            commit_id: None,
            activation: Some(ActivationState::Frontier),
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
    assert_eq!(lookup.activation, Some(ActivationState::Frontier));
    Ok(())
}
```

- [x] **Step 3: Add activation materialization test**

Add this test near other checkout materialization tests:

```rust
#[test]
fn checkout_filters_by_activation_state() -> Result<(), Box<dyn std::error::Error>> {
    let mut kernel = MemoryKernel::default();
    let active = sample_cell("project:continuitydb:activation-active", 0.91, 10)?;
    let mut frontier = sample_cell("project:continuitydb:activation-frontier", 0.9, 10)?;
    frontier.activation = ActivationState::Frontier;
    append_committed(&mut kernel, active)?;
    let frontier = append_committed(&mut kernel, frontier)?;

    let slice = checkout(
        &kernel,
        CheckoutRequest {
            scope: Some(Scope::Project("continuitydb".to_string())),
            valid_at: None,
            system_at: None,
            commit_id: None,
            activation: Some(ActivationState::Frontier),
            answerability_question: None,
            evidence_source: None,
            dependency_target: None,
            dependency_kind: None,
            minimum_confidence: Confidence::new(0.7)?,
            token_budget: 20,
        },
    )?;

    assert_eq!(slice.cells, vec![frontier]);
    Ok(())
}
```

- [x] **Step 4: Verify RED**

Run:

```bash
cargo test -p continuitydb-checkout activation --all-features
```

Expected: FAIL because `CheckoutRequest` has no `activation` field yet.

## Task 2: Checkout Implementation

**Files:**
- Modify: `crates/continuitydb-checkout/src/lib.rs`

- [x] **Step 1: Add request field**

Add this field after `commit_id` in `CheckoutRequest`:

```rust
/// Optional activation-state filter.
pub activation: Option<ActivationState>,
```

- [x] **Step 2: Push activation into storage lookup**

Add this field to the `CellLookup` in `checkout`:

```rust
activation: request.activation,
```

- [x] **Step 3: Verify GREEN**

Run:

```bash
cargo test -p continuitydb-checkout activation --all-features
cargo test -p continuitydb-checkout --all-features
```

Expected: PASS.

- [x] **Step 4: Commit checkout implementation**

```bash
git add crates/continuitydb-checkout/src/lib.rs docs/superpowers/plans/2026-05-20-activation-aware-checkout.md
git commit -m "feat: add activation-aware checkout"
```

## Task 3: Failing Query Tests

**Files:**
- Modify: `crates/continuitydb-query/src/lib.rs`

- [x] **Step 1: Add activation import**

Update the test import to include `ActivationState`:

```rust
use continuitydb_core::{
    ActivationState, CellDependencyKind, CommitId, Confidence, Scope, StateCellId,
};
```

- [x] **Step 2: Add default assertion**

In `minimal_checkout_query_compiles_task_answerability_and_defaults`, assert:

```rust
assert_eq!(request.activation, None);
```

- [x] **Step 3: Add typed query activation compile test**

Add this test after `checkout_query_compiles_evidence_and_dependency_requirements`:

```rust
#[test]
fn checkout_query_compiles_activation_requirement() -> Result<(), Box<dyn std::error::Error>> {
    let task = QueryTask::new("frontier-review", "what frontier cells need review?");
    let requirements = QueryRequirements {
        activation: Some(ActivationState::Frontier),
        ..QueryRequirements::default()
    };

    let request = CheckoutQuery::new(task)
        .with_requirements(requirements)
        .compile_checkout()?;

    assert_eq!(request.activation, Some(ActivationState::Frontier));
    Ok(())
}
```

- [x] **Step 4: Add text parser activation tests**

Add this test after `text_query_parses_dependency_constraints`:

```rust
#[test]
fn text_query_parses_activation_constraint() -> Result<(), Box<dyn std::error::Error>> {
    let query = parse_query_text(
        r#"CHECKOUT "frontier-review" ANSWER "what frontier cells need review?"
WHERE activation = frontier"#,
    )?;

    let ContinuityQuery::Checkout(checkout) = query;
    assert_eq!(
        checkout.requirements().activation,
        Some(ActivationState::Frontier)
    );
    Ok(())
}
```

Extend `text_query_rejects_invalid_values` with:

```rust
assert_eq!(
    parse_query_text(
        r#"CHECKOUT "release" ANSWER "what should ship?" WHERE activation = unknown"#
    ),
    Err(QueryTextError::InvalidValue)
);
```

- [x] **Step 5: Verify RED**

Run:

```bash
cargo test -p continuitydb-query activation --all-features
```

Expected: FAIL because `QueryRequirements` and the text parser do not support activation yet.

## Task 4: Query Implementation

**Files:**
- Modify: `crates/continuitydb-query/src/lib.rs`
- Modify: `crates/continuitydb-query/src/text.rs`

- [x] **Step 1: Add production import**

Update the production core import in `lib.rs`:

```rust
use continuitydb_core::{
    ActivationState, CellDependencyKind, CommitId, Confidence, Scope, StateCellId,
};
```

- [x] **Step 2: Add QueryRequirements field and default**

Add this field after `commit_id`:

```rust
/// Optional activation-state filter.
pub activation: Option<ActivationState>,
```

Set it to `None` in `Default`.

- [x] **Step 3: Compile activation**

Add this field in `CheckoutQuery::compile_checkout`:

```rust
activation: self.requirements.activation,
```

- [x] **Step 4: Add text parser import**

Update the core import in `text.rs`:

```rust
use continuitydb_core::{
    ActivationState, CellDependencyKind, CommitId, Confidence, Scope, StateCellId,
};
```

- [x] **Step 5: Parse activation constraint**

Add this branch after `commit_id`:

```rust
"activation" => {
    self.expect_token(Token::Eq)?;
    requirements.activation = Some(self.parse_activation()?);
}
```

Add this helper after `parse_commit_id`:

```rust
fn parse_activation(&mut self) -> Result<ActivationState, QueryTextError> {
    let ident = self.expect_ident()?;
    match ident.to_ascii_lowercase().as_str() {
        "dormant" => Ok(ActivationState::Dormant),
        "active" => Ok(ActivationState::Active),
        "frontier" => Ok(ActivationState::Frontier),
        "retired" => Ok(ActivationState::Retired),
        _ => Err(QueryTextError::InvalidValue),
    }
}
```

- [x] **Step 6: Verify GREEN**

Run:

```bash
cargo test -p continuitydb-query activation --all-features
cargo test -p continuitydb-query --all-features
```

Expected: PASS.

- [x] **Step 7: Commit query implementation**

```bash
git add crates/continuitydb-query/src/lib.rs crates/continuitydb-query/src/text.rs docs/superpowers/plans/2026-05-20-activation-aware-checkout.md
git commit -m "feat: add activation query constraints"
```

## Task 5: Documentation

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-activation-aware-checkout.md`

- [x] **Step 1: Update README current scope**

Add this bullet after deterministic checkout:

```markdown
- Activation-aware checkout and strict text query constraints.
```

- [x] **Step 2: Update Checkout roadmap**

Add this milestone after dependency-aware checkout constraints and renumber later checkout milestones:

```markdown
6. Add activation-aware checkout constraints. Implemented `CheckoutRequest.activation` with pushdown into `CellLookup.activation` so callers can materialize dormant, active, frontier, or retired StateCells through deterministic checkout.
```

- [x] **Step 3: Update Query Language roadmap**

Add this milestone after dependency constraints:

```markdown
10. Add activation constraints to typed and text checkout queries. Implemented `QueryRequirements.activation` and strict text `activation = ...` parsing so query files can materialize activation-scoped continuity slices.
```

- [x] **Step 4: Run full verification gate**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

Expected: all commands exit successfully.

- [x] **Step 5: Commit documentation**

```bash
git add README.md docs/roadmap.md docs/superpowers/plans/2026-05-20-activation-aware-checkout.md
git commit -m "docs: record activation-aware checkout"
```

## Self-Review

- Spec coverage: Tasks add checkout pushdown, materialization, typed query compilation, text parsing, docs, and full verification.
- Placeholder scan: No placeholders or vague implementation steps remain.
- Type consistency: `ActivationState`, `CheckoutRequest.activation`, `QueryRequirements.activation`, and `CellLookup.activation` match existing crate types.
