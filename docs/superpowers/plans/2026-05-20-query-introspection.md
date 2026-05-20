# Query Introspection Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add read-only accessors for typed checkout query AST values.

**Architecture:** `CheckoutQuery` keeps private fields and exposes stable accessor methods for task, requirements, return shape, and optimization. Existing builder, serde, envelope, and compile behavior remain unchanged.

**Tech Stack:** Rust 2021, `continuitydb-query`, `serde_json`.

---

## File Structure

- Modify `crates/continuitydb-query/src/lib.rs`: add accessor tests and read-only accessor methods.
- Modify `README.md`: add typed query introspection to current scope.
- Modify `docs/roadmap.md`: add Query Language milestone.
- Modify this plan: mark steps complete as executed.

## Task 1: Failing Accessor Tests

**Files:**
- Modify: `crates/continuitydb-query/src/lib.rs`

- [x] **Step 1: Add direct accessor test**

Add this test to the existing test module in `crates/continuitydb-query/src/lib.rs`:

```rust
#[test]
fn checkout_query_accessors_expose_typed_semantics() -> Result<(), Box<dyn std::error::Error>> {
    let valid_at = Utc
        .with_ymd_and_hms(2026, 5, 20, 10, 0, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let requirements = QueryRequirements {
        scope: Some(Scope::Project("continuitydb".to_string())),
        valid_at: Some(valid_at),
        minimum_confidence: Confidence::new(0.7)?,
        token_budget: 1200,
        ..QueryRequirements::default()
    };
    let query = CheckoutQuery::new(QueryTask::new("stored-facts", "what is stored?"))
        .with_requirements(requirements.clone())
        .with_return_shape(QueryReturnShape::CellsOnly)
        .with_optimization(QueryOptimization::TokenCostOnly);

    assert_eq!(query.task().name, "stored-facts");
    assert_eq!(query.task().answerability_question, "what is stored?");
    assert_eq!(query.requirements(), &requirements);
    assert_eq!(query.return_shape(), QueryReturnShape::CellsOnly);
    assert_eq!(query.optimization(), QueryOptimization::TokenCostOnly);
    Ok(())
}
```

- [x] **Step 2: Add envelope decode accessor test**

Add this test after the direct accessor test:

```rust
#[test]
fn decoded_query_envelope_can_be_inspected_before_compile(
) -> Result<(), Box<dyn std::error::Error>> {
    let query = ContinuityQuery::Checkout(
        CheckoutQuery::new(QueryTask::new("stored-facts", "what is stored?"))
            .with_return_shape(QueryReturnShape::CellsOnly),
    );
    let encoded = encode_query_json(query)?;
    let decoded = decode_query_json(&encoded)?;

    let ContinuityQuery::Checkout(checkout) = decoded;
    assert_eq!(checkout.task().name, "stored-facts");
    assert_eq!(checkout.return_shape(), QueryReturnShape::CellsOnly);
    assert_eq!(
        checkout.compile_checkout(),
        Err(QueryError::UnsupportedReturnShape(QueryReturnShape::CellsOnly))
    );
    Ok(())
}
```

- [x] **Step 3: Verify RED**

Run:

```bash
cargo test -p continuitydb-query accessor --all-features
```

Expected: FAIL because `CheckoutQuery` does not have accessor methods.

## Task 2: Accessor Implementation

**Files:**
- Modify: `crates/continuitydb-query/src/lib.rs`

- [x] **Step 1: Add accessor methods**

Add these methods inside the existing `impl CheckoutQuery` block after `new`:

```rust
    /// Returns query task identity and answerability intent.
    pub fn task(&self) -> &QueryTask {
        &self.task
    }

    /// Returns deterministic checkout requirements.
    pub fn requirements(&self) -> &QueryRequirements {
        &self.requirements
    }

    /// Returns the requested materialization shape.
    pub fn return_shape(&self) -> QueryReturnShape {
        self.return_shape
    }

    /// Returns the requested optimization policy.
    pub fn optimization(&self) -> QueryOptimization {
        self.optimization
    }
```

- [x] **Step 2: Verify GREEN**

Run:

```bash
cargo test -p continuitydb-query accessor --all-features
cargo test -p continuitydb-query --all-features
```

Expected: PASS.

- [x] **Step 3: Commit implementation**

```bash
git add crates/continuitydb-query/src/lib.rs docs/superpowers/plans/2026-05-20-query-introspection.md
git commit -m "feat: add query ast accessors"
```

## Task 3: Docs and Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-query-introspection.md`

- [x] **Step 1: Update README**

Add this current-scope bullet near the existing query bullets:

```markdown
- Read-only typed query AST introspection for embedders and bindings.
```

- [x] **Step 2: Update roadmap**

Add this Query Language milestone after the query envelope milestone:

```markdown
6. Add read-only typed query AST introspection. Implemented `CheckoutQuery` accessors for task, requirements, return shape, and optimization so embedders and bindings can inspect decoded query files without exposing internal fields.
```

- [x] **Step 3: Run full verification gate**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

Expected: all commands exit 0.

- [x] **Step 4: Commit docs**

```bash
git add README.md docs/roadmap.md docs/superpowers/plans/2026-05-20-query-introspection.md
git commit -m "docs: record query introspection"
```
