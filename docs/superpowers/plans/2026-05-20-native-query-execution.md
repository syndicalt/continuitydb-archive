# Native Query Execution Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let embedders execute typed Continuity queries directly through `ContinuityDb<K>`.

**Architecture:** `continuitydb-api` depends on `continuitydb-query`, wraps `QueryError` in `ContinuityError`, compiles typed queries into `CheckoutRequest`, and delegates to the existing checkout operation.

**Tech Stack:** Rust 2021, `continuitydb-api`, `continuitydb-query`, `continuitydb-checkout`, `continuitydb-memory`, TDD unit tests.

---

## File Structure

- Modify `crates/continuitydb-api/Cargo.toml`: add `continuitydb-query`.
- Modify `crates/continuitydb-api/src/lib.rs`: add query error wrapping, query execution methods, and unit tests.
- Modify `README.md`: add native query execution to current scope.
- Modify `docs/roadmap.md`: add Query Language and Native API milestones.
- Modify this plan: mark steps complete as executed.

## Task 1: API Query Execution Tests

**Files:**
- Modify: `crates/continuitydb-api/Cargo.toml`
- Modify: `crates/continuitydb-api/src/lib.rs`

- [x] **Step 1: Add API dependency on query crate**

Add this dependency to `crates/continuitydb-api/Cargo.toml`:

```toml
continuitydb-query = { path = "../continuitydb-query" }
```

- [x] **Step 2: Add failing API tests**

In `crates/continuitydb-api/src/lib.rs`, add this import in the test module:

```rust
use continuitydb_query::{
    CheckoutQuery, ContinuityQuery, QueryError, QueryOptimization, QueryRequirements,
    QueryReturnShape, QueryTask,
};
```

Add these tests near `api_ingests_and_checkouts_cells`:

```rust
#[test]
fn api_checkout_query_materializes_slice() -> Result<(), Box<dyn std::error::Error>> {
    let committed_at = Utc
        .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let mut db = ContinuityDb::new(MemoryKernel::default());
    let cell = sample_cell("project:continuitydb:api-query", 0.91, 12)?;
    let cell_id = cell.id;
    db.ingest_cell_at(cell, committed_at)?;

    let query = CheckoutQuery::new(QueryTask::new(
        "api-query",
        "what should the agent know?",
    ))
    .with_requirements(QueryRequirements {
        scope: Some(Scope::Project("continuitydb".to_string())),
        valid_at: Some(committed_at),
        system_at: Some(committed_at),
        minimum_confidence: Confidence::new(0.8)?,
        token_budget: 100,
        ..QueryRequirements::default()
    });
    let slice = db.checkout_query(query)?;

    assert_eq!(slice.cells.len(), 1);
    assert_eq!(slice.cells[0].id, cell_id);
    Ok(())
}

#[test]
fn api_checkout_query_applies_token_budget_to_alternatives(
) -> Result<(), Box<dyn std::error::Error>> {
    let committed_at = Utc
        .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let mut db = ContinuityDb::new(MemoryKernel::default());
    let first = sample_cell("project:continuitydb:api-query-budget-first", 0.91, 80)?;
    let second = sample_cell("project:continuitydb:api-query-budget-second", 0.9, 80)?;
    db.ingest_cells_at(vec![first, second], committed_at)?;

    let query = CheckoutQuery::new(QueryTask::new(
        "api-query-budget",
        "what should the agent know?",
    ))
    .with_requirements(QueryRequirements {
        scope: Some(Scope::Project("continuitydb".to_string())),
        minimum_confidence: Confidence::new(0.8)?,
        token_budget: 100,
        ..QueryRequirements::default()
    });
    let slice = db.checkout_query(query)?;

    assert_eq!(slice.cells.len(), 1);
    assert_eq!(slice.alternatives.len(), 1);
    Ok(())
}

#[test]
fn api_checkout_continuity_query_delegates_top_level_query(
) -> Result<(), Box<dyn std::error::Error>> {
    let committed_at = Utc
        .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let mut db = ContinuityDb::new(MemoryKernel::default());
    let cell = sample_cell("project:continuitydb:api-continuity-query", 0.91, 12)?;
    let cell_id = cell.id;
    db.ingest_cell_at(cell, committed_at)?;

    let query = ContinuityQuery::Checkout(CheckoutQuery::new(QueryTask::new(
        "api-continuity-query",
        "what should the agent know?",
    )));
    let slice = db.checkout_continuity_query(query)?;

    assert_eq!(slice.cells.len(), 1);
    assert_eq!(slice.cells[0].id, cell_id);
    Ok(())
}

#[test]
fn api_checkout_query_returns_unsupported_shape_error() {
    let db = ContinuityDb::new(MemoryKernel::default());
    let query = CheckoutQuery::new(QueryTask::new(
        "api-query-shape",
        "what should the agent know?",
    ))
    .with_return_shape(QueryReturnShape::CellsOnly);

    let result = db.checkout_query(query);

    assert!(matches!(
        result,
        Err(ContinuityError::Query(QueryError::UnsupportedReturnShape(
            QueryReturnShape::CellsOnly
        )))
    ));
}

#[test]
fn api_checkout_query_returns_unsupported_optimization_error() {
    let db = ContinuityDb::new(MemoryKernel::default());
    let query = CheckoutQuery::new(QueryTask::new(
        "api-query-optimization",
        "what should the agent know?",
    ))
    .with_optimization(QueryOptimization::TokenCostOnly);

    let result = db.checkout_query(query);

    assert!(matches!(
        result,
        Err(ContinuityError::Query(QueryError::UnsupportedOptimization(
            QueryOptimization::TokenCostOnly
        )))
    ));
}
```

- [x] **Step 3: Run tests and verify RED**

Run:

```bash
cargo test -p continuitydb-api checkout_query --all-features
```

Expected: FAIL because `ContinuityError::Query`, `checkout_query`, and `checkout_continuity_query` do not exist.

## Task 2: API Query Execution Implementation

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`

- [x] **Step 1: Add query imports and error variant**

At the top of `crates/continuitydb-api/src/lib.rs`, add:

```rust
use continuitydb_query::{CheckoutQuery, ContinuityQuery, QueryError};
```

Add this `ContinuityError` variant:

```rust
/// Query compilation failure.
#[error(transparent)]
Query(#[from] QueryError),
```

- [x] **Step 2: Add API methods**

Add these methods near `checkout` in `impl<K: StorageKernel> ContinuityDb<K>`:

```rust
/// Materializes a deterministic continuity slice from a typed checkout query.
pub fn checkout_query(&self, query: CheckoutQuery) -> Result<CheckoutSlice, ContinuityError> {
    self.checkout(query.compile_checkout()?)
}

/// Materializes a deterministic continuity slice from a top-level typed query.
pub fn checkout_continuity_query(
    &self,
    query: ContinuityQuery,
) -> Result<CheckoutSlice, ContinuityError> {
    self.checkout(query.compile_checkout()?)
}
```

- [x] **Step 3: Run focused tests and verify GREEN**

Run:

```bash
cargo test -p continuitydb-api checkout_query --all-features
cargo test -p continuitydb-api checkout_continuity_query --all-features
```

Expected: PASS.

- [x] **Step 4: Commit implementation**

```bash
git add crates/continuitydb-api/Cargo.toml crates/continuitydb-api/src/lib.rs Cargo.lock docs/superpowers/plans/2026-05-20-native-query-execution.md
git commit -m "feat: execute typed queries through native api"
```

## Task 3: Docs and Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-native-query-execution.md`

- [x] **Step 1: Update README**

Add this current-scope bullet near the query AST bullet:

```markdown
- Native typed query execution through the embeddable API.
```

- [x] **Step 2: Update roadmap**

Add this Query Language milestone after the typed AST milestone:

```markdown
2. Execute typed queries through the native API. Implemented `ContinuityDb::checkout_query` and `checkout_continuity_query` so embedders can materialize typed Continuity queries without manually compiling them into checkout requests.
```

Add this Native API milestone after typed embeddable operations:

```markdown
2. Add native typed query execution. Implemented checkout execution for `continuitydb-query` AST values through `ContinuityDb<K>`, preserving typed query errors and delegating materialization to the existing checkout engine.
```

Renumber the following Native API milestones.

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
git add README.md docs/roadmap.md docs/superpowers/plans/2026-05-20-native-query-execution.md
git commit -m "docs: record native query execution"
```
