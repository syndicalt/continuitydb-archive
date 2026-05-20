# Continuity Query AST Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a typed `continuitydb-query` crate that represents checkout queries as Rust data and compiles them into `CheckoutRequest`.

**Architecture:** The query crate owns only query-facing AST types and a narrow compiler into `continuitydb-checkout`. It does not parse text, execute storage, or add new checkout behavior.

**Tech Stack:** Rust 2021, workspace crates, `continuitydb-core`, `continuitydb-checkout`, `thiserror`, unit tests.

---

## File Structure

- Modify `Cargo.toml`: add `crates/continuitydb-query` to the workspace members.
- Create `crates/continuitydb-query/Cargo.toml`: crate metadata and dependencies.
- Create `crates/continuitydb-query/src/lib.rs`: query AST, compiler, errors, and unit tests.
- Modify `README.md`: add the query AST to current scope.
- Modify `docs/roadmap.md`: add a Query Language milestone.
- Modify this plan: mark steps complete as executed.

## Task 1: Query Crate Scaffold

**Files:**
- Modify: `Cargo.toml`
- Create: `crates/continuitydb-query/Cargo.toml`
- Create: `crates/continuitydb-query/src/lib.rs`

- [x] **Step 1: Add the crate to the workspace**

Add the new member near `continuitydb-checkout` in the root `Cargo.toml`:

```toml
    "crates/continuitydb-checkout",
    "crates/continuitydb-query",
```

- [x] **Step 2: Create crate manifest**

Create `crates/continuitydb-query/Cargo.toml`:

```toml
[package]
name = "continuitydb-query"
version = "0.1.0"
edition.workspace = true
license.workspace = true
repository.workspace = true
rust-version.workspace = true

[dependencies]
chrono.workspace = true
continuitydb-checkout = { path = "../continuitydb-checkout" }
continuitydb-core = { path = "../continuitydb-core" }
thiserror.workspace = true

[lints]
workspace = true
```

- [x] **Step 3: Create temporary crate root**

Create `crates/continuitydb-query/src/lib.rs`:

```rust
//! Typed ContinuityDB query AST.
```

- [x] **Step 4: Verify the empty crate builds**

Run:

```bash
cargo test -p continuitydb-query
```

Expected: PASS with zero tests.

- [x] **Step 5: Commit scaffold**

```bash
git add Cargo.toml crates/continuitydb-query
git commit -m "chore: add continuity query crate"
```

## Task 2: Failing Query Compiler Tests

**Files:**
- Modify: `crates/continuitydb-query/src/lib.rs`

- [x] **Step 1: Add tests before implementation**

Replace `crates/continuitydb-query/src/lib.rs` with this test-first skeleton:

```rust
//! Typed ContinuityDB query AST.

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use continuitydb_core::{
        CellDependencyKind, CommitId, Confidence, Scope, StateCellId,
    };

    #[test]
    fn minimal_checkout_query_compiles_task_answerability_and_defaults() -> Result<(), Box<dyn std::error::Error>> {
        let task = QueryTask::new(
            "release-readiness",
            "what is the release status?",
        );
        let request = CheckoutQuery::new(task).compile_checkout()?;

        assert_eq!(
            request.answerability_question.as_deref(),
            Some("what is the release status?")
        );
        assert_eq!(request.scope, None);
        assert_eq!(request.valid_at, None);
        assert_eq!(request.system_at, None);
        assert_eq!(request.commit_id, None);
        assert_eq!(request.evidence_source, None);
        assert_eq!(request.dependency_target, None);
        assert_eq!(request.dependency_kind, None);
        assert_eq!(request.minimum_confidence, Confidence::new(0.0)?);
        assert_eq!(request.token_budget, i64::MAX);
        Ok(())
    }

    #[test]
    fn checkout_query_compiles_temporal_confidence_scope_and_budget_requirements() -> Result<(), Box<dyn std::error::Error>> {
        let valid_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 10, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let system_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 11, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let commit_id = CommitId::new();
        let task = QueryTask::new("release-readiness", "what is the release status?");
        let requirements = QueryRequirements {
            scope: Some(Scope::Project("continuitydb".to_string())),
            valid_at: Some(valid_at),
            system_at: Some(system_at),
            commit_id: Some(commit_id),
            minimum_confidence: Confidence::new(0.7)?,
            token_budget: 1200,
            ..QueryRequirements::default()
        };

        let request = CheckoutQuery::new(task)
            .with_requirements(requirements)
            .compile_checkout()?;

        assert_eq!(request.scope, Some(Scope::Project("continuitydb".to_string())));
        assert_eq!(request.valid_at, Some(valid_at));
        assert_eq!(request.system_at, Some(system_at));
        assert_eq!(request.commit_id, Some(commit_id));
        assert_eq!(request.minimum_confidence, Confidence::new(0.7)?);
        assert_eq!(request.token_budget, 1200);
        Ok(())
    }

    #[test]
    fn checkout_query_compiles_evidence_and_dependency_requirements() -> Result<(), Box<dyn std::error::Error>> {
        let dependency_target = StateCellId::new();
        let task = QueryTask::new("audit-risk", "what risks depend on this evidence?");
        let requirements = QueryRequirements {
            evidence_source: Some("source:incident-review".to_string()),
            dependency_target: Some(dependency_target),
            dependency_kind: Some(CellDependencyKind::DependsOn),
            ..QueryRequirements::default()
        };

        let request = CheckoutQuery::new(task)
            .with_requirements(requirements)
            .compile_checkout()?;

        assert_eq!(
            request.evidence_source.as_deref(),
            Some("source:incident-review")
        );
        assert_eq!(request.dependency_target, Some(dependency_target));
        assert_eq!(request.dependency_kind, Some(CellDependencyKind::DependsOn));
        Ok(())
    }

    #[test]
    fn continuity_query_delegates_checkout_compilation() -> Result<(), Box<dyn std::error::Error>> {
        let query = ContinuityQuery::Checkout(CheckoutQuery::new(QueryTask::new(
            "release-readiness",
            "what is the release status?",
        )));

        let request = query.compile_checkout()?;

        assert_eq!(
            request.answerability_question.as_deref(),
            Some("what is the release status?")
        );
        Ok(())
    }

    #[test]
    fn unsupported_return_shape_fails_with_typed_error() -> Result<(), Box<dyn std::error::Error>> {
        let result = CheckoutQuery::new(QueryTask::new(
            "release-readiness",
            "what is the release status?",
        ))
        .with_return_shape(QueryReturnShape::CellsOnly)
        .compile_checkout();

        let error = result
            .err()
            .ok_or_else(|| std::io::Error::other("unsupported return shape should fail"))?;
        assert_eq!(
            error,
            QueryError::UnsupportedReturnShape(QueryReturnShape::CellsOnly)
        );
        Ok(())
    }

    #[test]
    fn unsupported_optimization_fails_with_typed_error() -> Result<(), Box<dyn std::error::Error>> {
        let result = CheckoutQuery::new(QueryTask::new(
            "release-readiness",
            "what is the release status?",
        ))
        .with_optimization(QueryOptimization::TokenCostOnly)
        .compile_checkout();

        let error = result
            .err()
            .ok_or_else(|| std::io::Error::other("unsupported optimization should fail"))?;
        assert_eq!(
            error,
            QueryError::UnsupportedOptimization(QueryOptimization::TokenCostOnly)
        );
        Ok(())
    }
}
```

- [x] **Step 2: Run tests and verify RED**

Run:

```bash
cargo test -p continuitydb-query
```

Expected: FAIL because `QueryTask`, `CheckoutQuery`, `ContinuityQuery`, `QueryRequirements`, `QueryReturnShape`, `QueryOptimization`, and `QueryError` do not exist.

## Task 3: Query AST Implementation

**Files:**
- Modify: `crates/continuitydb-query/src/lib.rs`

- [x] **Step 1: Add production types and compiler above the tests**

Insert this code above the `#[cfg(test)]` module:

```rust
use chrono::{DateTime, Utc};
use continuitydb_checkout::CheckoutRequest;
use continuitydb_core::{CellDependencyKind, CommitId, Confidence, Scope, StateCellId};
use thiserror::Error;

/// Top-level typed ContinuityDB query.
#[derive(Clone, Debug, PartialEq)]
pub enum ContinuityQuery {
    /// Materialize a continuity checkout slice.
    Checkout(CheckoutQuery),
}

impl ContinuityQuery {
    /// Compiles this query into a deterministic checkout request.
    pub fn compile_checkout(self) -> Result<CheckoutRequest, QueryError> {
        match self {
            Self::Checkout(query) => query.compile_checkout(),
        }
    }
}

/// Structured checkout query.
#[derive(Clone, Debug, PartialEq)]
pub struct CheckoutQuery {
    task: QueryTask,
    requirements: QueryRequirements,
    return_shape: QueryReturnShape,
    optimization: QueryOptimization,
}

impl CheckoutQuery {
    /// Creates a checkout query with default requirements and supported output semantics.
    pub fn new(task: QueryTask) -> Self {
        Self {
            task,
            requirements: QueryRequirements::default(),
            return_shape: QueryReturnShape::PackedContextWithMetadata,
            optimization: QueryOptimization::DeterministicUtility,
        }
    }

    /// Replaces deterministic checkout requirements.
    pub fn with_requirements(mut self, requirements: QueryRequirements) -> Self {
        self.requirements = requirements;
        self
    }

    /// Replaces the requested materialization shape.
    pub fn with_return_shape(mut self, return_shape: QueryReturnShape) -> Self {
        self.return_shape = return_shape;
        self
    }

    /// Replaces the requested optimization policy.
    pub fn with_optimization(mut self, optimization: QueryOptimization) -> Self {
        self.optimization = optimization;
        self
    }

    /// Compiles this query into the current deterministic checkout request type.
    pub fn compile_checkout(self) -> Result<CheckoutRequest, QueryError> {
        if self.return_shape != QueryReturnShape::PackedContextWithMetadata {
            return Err(QueryError::UnsupportedReturnShape(self.return_shape));
        }
        if self.optimization != QueryOptimization::DeterministicUtility {
            return Err(QueryError::UnsupportedOptimization(self.optimization));
        }

        Ok(CheckoutRequest {
            scope: self.requirements.scope,
            valid_at: self.requirements.valid_at,
            system_at: self.requirements.system_at,
            commit_id: self.requirements.commit_id,
            answerability_question: Some(self.task.answerability_question),
            evidence_source: self.requirements.evidence_source,
            dependency_target: self.requirements.dependency_target,
            dependency_kind: self.requirements.dependency_kind,
            minimum_confidence: self.requirements.minimum_confidence,
            token_budget: self.requirements.token_budget,
        })
    }
}

/// Query task identity and answerability intent.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueryTask {
    /// Stable task name or identifier.
    pub name: String,
    /// Exact question or intent selected StateCells should answer.
    pub answerability_question: String,
}

impl QueryTask {
    /// Creates a query task.
    pub fn new(
        name: impl Into<String>,
        answerability_question: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            answerability_question: answerability_question.into(),
        }
    }
}

/// Deterministic requirements accepted by the first checkout query compiler.
#[derive(Clone, Debug, PartialEq)]
pub struct QueryRequirements {
    /// Optional scope filter.
    pub scope: Option<Scope>,
    /// Optional valid-time filter.
    pub valid_at: Option<DateTime<Utc>>,
    /// Optional system transaction-time filter.
    pub system_at: Option<DateTime<Utc>>,
    /// Optional database commit identifier filter.
    pub commit_id: Option<CommitId>,
    /// Optional exact evidence-source filter.
    pub evidence_source: Option<String>,
    /// Optional dependency target filter.
    pub dependency_target: Option<StateCellId>,
    /// Optional dependency kind filter.
    pub dependency_kind: Option<CellDependencyKind>,
    /// Minimum evidence confidence for included cells.
    pub minimum_confidence: Confidence,
    /// Maximum token budget for the returned slice.
    pub token_budget: i64,
}

impl Default for QueryRequirements {
    fn default() -> Self {
        Self {
            scope: None,
            valid_at: None,
            system_at: None,
            commit_id: None,
            evidence_source: None,
            dependency_target: None,
            dependency_kind: None,
            minimum_confidence: zero_confidence(),
            token_budget: i64::MAX,
        }
    }
}

fn zero_confidence() -> Confidence {
    match Confidence::new(0.0) {
        Ok(confidence) => confidence,
        Err(_) => Confidence::default(),
    }
}

/// Requested checkout materialization shape.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QueryReturnShape {
    /// Current full checkout slice shape.
    PackedContextWithMetadata,
    /// Future cell-only projection.
    CellsOnly,
}

/// Requested checkout optimization policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QueryOptimization {
    /// Current deterministic utility-aware ranking.
    DeterministicUtility,
    /// Future cost-only packing.
    TokenCostOnly,
}

/// Query compilation errors.
#[derive(Debug, Error, PartialEq)]
pub enum QueryError {
    /// The requested return shape is not supported by this compiler.
    #[error("unsupported return shape: {0:?}")]
    UnsupportedReturnShape(QueryReturnShape),
    /// The requested optimization policy is not supported by this compiler.
    #[error("unsupported optimization: {0:?}")]
    UnsupportedOptimization(QueryOptimization),
}
```

- [x] **Step 2: Run tests and verify GREEN**

Run:

```bash
cargo test -p continuitydb-query
```

Expected: PASS.

- [ ] **Step 3: Commit query AST implementation**

```bash
git add crates/continuitydb-query/src/lib.rs
git commit -m "feat: add continuity query ast"
```

## Task 4: Docs and Roadmap

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-continuity-query-ast.md`

- [ ] **Step 1: Update README current scope**

Add this bullet near deterministic checkout:

```markdown
- Typed Continuity Query AST compiling checkout semantics into native requests.
```

- [ ] **Step 2: Update roadmap**

Add this section after Checkout Milestones:

```markdown
## Query Language Milestones

1. Add a typed Continuity Query AST. Implemented `continuitydb-query` with structured checkout query types and compilation into `CheckoutRequest`, establishing the semantic target for future text syntax and API bindings.
```

- [ ] **Step 3: Run full verification gate**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

Expected: all commands exit 0.

- [ ] **Step 4: Commit docs**

```bash
git add README.md docs/roadmap.md docs/superpowers/plans/2026-05-20-continuity-query-ast.md
git commit -m "docs: record continuity query ast"
```
