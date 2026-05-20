# Query AST Serde Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add serde serialization and deserialization for the typed Continuity Query AST.

**Architecture:** Derive serde traits directly on query AST types, use snake-case enum variant names for external stability, and keep compile-time query semantics unchanged.

**Tech Stack:** Rust 2021, `continuitydb-query`, `serde`, `serde_json` dev tests.

---

## File Structure

- Modify `crates/continuitydb-query/Cargo.toml`: add serde dependency and serde_json dev dependency.
- Modify `crates/continuitydb-query/src/lib.rs`: derive `Serialize`/`Deserialize`, add serde enum casing, and add round-trip tests.
- Modify `README.md`: add portable typed query serialization to current scope.
- Modify `docs/roadmap.md`: add Query Language milestone.
- Modify this plan: mark steps complete as executed.

## Task 1: Failing Serde Tests

**Files:**
- Modify: `crates/continuitydb-query/Cargo.toml`
- Modify: `crates/continuitydb-query/src/lib.rs`

- [x] **Step 1: Add serde_json as a dev dependency**

Add this to `crates/continuitydb-query/Cargo.toml`:

```toml
[dev-dependencies]
serde_json.workspace = true
```

- [x] **Step 2: Add failing serialization tests**

Add these tests to the existing test module in `crates/continuitydb-query/src/lib.rs`:

```rust
#[test]
fn checkout_query_round_trips_json_and_compiles() -> Result<(), Box<dyn std::error::Error>> {
    let valid_at = Utc
        .with_ymd_and_hms(2026, 5, 20, 10, 0, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let task = QueryTask::new("release-readiness", "what is the release status?");
    let query = CheckoutQuery::new(task).with_requirements(QueryRequirements {
        scope: Some(Scope::Project("continuitydb".to_string())),
        valid_at: Some(valid_at),
        minimum_confidence: Confidence::new(0.7)?,
        token_budget: 1200,
        ..QueryRequirements::default()
    });

    let encoded = serde_json::to_vec(&query)?;
    let decoded: CheckoutQuery = serde_json::from_slice(&encoded)?;
    let request = decoded.compile_checkout()?;

    assert_eq!(request.scope, Some(Scope::Project("continuitydb".to_string())));
    assert_eq!(request.valid_at, Some(valid_at));
    assert_eq!(request.minimum_confidence, Confidence::new(0.7)?);
    assert_eq!(request.token_budget, 1200);
    Ok(())
}

#[test]
fn continuity_query_serializes_with_snake_case_checkout_tag(
) -> Result<(), Box<dyn std::error::Error>> {
    let query = ContinuityQuery::Checkout(CheckoutQuery::new(QueryTask::new(
        "release-readiness",
        "what is the release status?",
    )));

    let value = serde_json::to_value(&query)?;
    let decoded: ContinuityQuery = serde_json::from_value(value.clone())?;

    assert!(value.get("checkout").is_some());
    assert_eq!(decoded, query);
    Ok(())
}

#[test]
fn unsupported_query_semantics_survive_json_round_trip(
) -> Result<(), Box<dyn std::error::Error>> {
    let query = CheckoutQuery::new(QueryTask::new(
        "release-readiness",
        "what is the release status?",
    ))
    .with_return_shape(QueryReturnShape::CellsOnly)
    .with_optimization(QueryOptimization::TokenCostOnly);

    let value = serde_json::to_value(&query)?;
    assert_eq!(
        value["return_shape"].as_str(),
        Some("cells_only")
    );
    assert_eq!(
        value["optimization"].as_str(),
        Some("token_cost_only")
    );

    let decoded: CheckoutQuery = serde_json::from_value(value)?;
    let error = decoded
        .compile_checkout()
        .err()
        .ok_or_else(|| std::io::Error::other("unsupported return shape should fail"))?;

    assert_eq!(
        error,
        QueryError::UnsupportedReturnShape(QueryReturnShape::CellsOnly)
    );
    Ok(())
}
```

- [x] **Step 3: Run tests and verify RED**

Run:

```bash
cargo test -p continuitydb-query serde --all-features
```

Expected: FAIL because query AST types do not implement serde traits.

## Task 2: Serde Implementation

**Files:**
- Modify: `crates/continuitydb-query/Cargo.toml`
- Modify: `crates/continuitydb-query/src/lib.rs`

- [x] **Step 1: Add serde dependency**

Add this dependency to `crates/continuitydb-query/Cargo.toml`:

```toml
serde.workspace = true
```

- [x] **Step 2: Add serde imports and derives**

In `crates/continuitydb-query/src/lib.rs`, add:

```rust
use serde::{Deserialize, Serialize};
```

Update derives and enum serde attributes:

```rust
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContinuityQuery { ... }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CheckoutQuery { ... }

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct QueryTask { ... }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QueryRequirements { ... }

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueryReturnShape { ... }

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueryOptimization { ... }
```

- [x] **Step 3: Run focused tests and verify GREEN**

Run:

```bash
cargo test -p continuitydb-query serde --all-features
cargo test -p continuitydb-query --all-features
```

Expected: PASS.

- [x] **Step 4: Commit implementation**

```bash
git add crates/continuitydb-query/Cargo.toml crates/continuitydb-query/src/lib.rs Cargo.lock docs/superpowers/plans/2026-05-20-query-ast-serde.md
git commit -m "feat: add query ast serde"
```

## Task 3: Docs and Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-query-ast-serde.md`

- [x] **Step 1: Update README**

Add this current-scope bullet near the query AST bullet:

```markdown
- Portable typed query serialization for bindings and future query files.
```

- [x] **Step 2: Update roadmap**

Add this Query Language milestone after native typed query execution:

```markdown
3. Add portable typed query serialization. Implemented serde support for `continuitydb-query` AST values with stable snake-case enum tags so future CLI query files, bindings, and agent APIs can exchange typed queries without a text parser.
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
git add README.md docs/roadmap.md docs/superpowers/plans/2026-05-20-query-ast-serde.md
git commit -m "docs: record query ast serde"
```
