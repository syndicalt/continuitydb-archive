# Text Query Dependency Constraints Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add dependency-aware constraints to strict text `CHECKOUT` queries.

**Architecture:** `continuitydb-core` first gives `StateCellId` stable text display and parsing, matching `CommitId`. `continuitydb-query` then parses quoted `dependency_target` UUID text and unquoted `dependency_kind` identifiers into the existing `QueryRequirements` fields before normal checkout compilation.

**Tech Stack:** Rust 2021, `continuitydb-core`, `continuitydb-query`, existing text parser, existing `CellDependencyKind` and `StateCellId` domain types.

---

## File Structure

- Modify `crates/continuitydb-core/src/cell.rs`: add `Display` and `FromStr` implementations for `StateCellId`.
- Modify `crates/continuitydb-core/src/lib.rs`: add core ID text round-trip tests.
- Modify `crates/continuitydb-query/src/text.rs`: import `CellDependencyKind` and `StateCellId`; add parser branches and helpers.
- Modify `crates/continuitydb-query/src/lib.rs`: add parser tests.
- Modify `README.md`: record dependency-aware strict text query constraints.
- Modify `docs/roadmap.md`: add Query Language milestone.
- Modify this plan: mark steps complete as executed.

## Task 1: Failing Core ID Tests

**Files:**
- Modify: `crates/continuitydb-core/src/lib.rs`

- [x] **Step 1: Add StateCellId text round-trip tests**

Add these tests near the existing commit ID text tests:

```rust
#[test]
fn state_cell_id_displays_and_parses_uuid_text() -> Result<(), Box<dyn std::error::Error>> {
    let cell_id = StateCellId::new();
    let text = cell_id.to_string();

    let parsed: StateCellId = text.parse()?;

    assert_eq!(parsed, cell_id);
    Ok(())
}

#[test]
fn state_cell_id_rejects_invalid_uuid_text() {
    let result = "not-a-uuid".parse::<StateCellId>();

    assert!(result.is_err());
}
```

- [x] **Step 2: Verify RED**

Run:

```bash
cargo test -p continuitydb-core state_cell_id_ --all-features
```

Expected: FAIL because `StateCellId` does not implement `Display` or `FromStr`.

## Task 2: Core ID Implementation

**Files:**
- Modify: `crates/continuitydb-core/src/cell.rs`

- [x] **Step 1: Add StateCellId display and parsing**

Add these implementations after `impl Default for StateCellId`:

```rust
impl fmt::Display for StateCellId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for StateCellId {
    type Err = uuid::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(value).map(Self)
    }
}
```

- [x] **Step 2: Verify GREEN**

Run:

```bash
cargo test -p continuitydb-core state_cell_id_ --all-features
cargo test -p continuitydb-core --all-features
```

Expected: PASS.

- [ ] **Step 3: Commit core implementation**

```bash
git add crates/continuitydb-core/src/cell.rs crates/continuitydb-core/src/lib.rs docs/superpowers/plans/2026-05-20-text-query-dependency-constraints.md
git commit -m "feat: add state cell id text parsing"
```

## Task 3: Failing Query Parser Tests

**Files:**
- Modify: `crates/continuitydb-query/src/lib.rs`

- [ ] **Step 1: Add dependency constraint parser test**

Add this test after `text_query_parses_temporal_and_commit_constraints`:

```rust
#[test]
fn text_query_parses_dependency_constraints() -> Result<(), Box<dyn std::error::Error>> {
    let dependency_target = StateCellId::new();
    let query = parse_query_text(&format!(
        r#"CHECKOUT "release" ANSWER "what should ship?"
WHERE dependency_target = "{dependency_target}"
  AND dependency_kind = derived_from"#
    ))?;

    let ContinuityQuery::Checkout(checkout) = query;
    assert_eq!(
        checkout.requirements().dependency_target,
        Some(dependency_target)
    );
    assert_eq!(
        checkout.requirements().dependency_kind,
        Some(CellDependencyKind::DerivedFrom)
    );
    Ok(())
}
```

- [ ] **Step 2: Add invalid dependency value tests**

Extend `text_query_rejects_invalid_values` with:

```rust
assert_eq!(
    parse_query_text(
        r#"CHECKOUT "release" ANSWER "what should ship?" WHERE dependency_target = "not-a-uuid""#
    ),
    Err(QueryTextError::InvalidValue)
);
assert_eq!(
    parse_query_text(
        r#"CHECKOUT "release" ANSWER "what should ship?" WHERE dependency_kind = unknown_kind"#
    ),
    Err(QueryTextError::InvalidValue)
);
```

- [ ] **Step 3: Verify RED**

Run:

```bash
cargo test -p continuitydb-query text_query_ --all-features
```

Expected: FAIL because dependency constraints are not supported in text queries.

## Task 4: Query Parser Implementation

**Files:**
- Modify: `crates/continuitydb-query/src/text.rs`

- [ ] **Step 1: Add imports**

Change the core import at the top of `text.rs` to:

```rust
use continuitydb_core::{CellDependencyKind, CommitId, Confidence, Scope, StateCellId};
```

- [ ] **Step 2: Add constraint branches**

Add these branches after `commit_id`:

```rust
"dependency_target" => {
    self.expect_token(Token::Eq)?;
    requirements.dependency_target = Some(self.parse_state_cell_id()?);
}
"dependency_kind" => {
    self.expect_token(Token::Eq)?;
    requirements.dependency_kind = Some(self.parse_dependency_kind()?);
}
```

- [ ] **Step 3: Add parse helpers**

Add these methods after `parse_commit_id`:

```rust
fn parse_state_cell_id(&mut self) -> Result<StateCellId, QueryTextError> {
    self.expect_string()?
        .parse::<StateCellId>()
        .map_err(|_error| QueryTextError::InvalidValue)
}

fn parse_dependency_kind(&mut self) -> Result<CellDependencyKind, QueryTextError> {
    let ident = self.expect_ident()?;
    match ident.to_ascii_lowercase().as_str() {
        "depends_on" => Ok(CellDependencyKind::DependsOn),
        "caused_by" => Ok(CellDependencyKind::CausedBy),
        "supports" => Ok(CellDependencyKind::Supports),
        "derived_from" => Ok(CellDependencyKind::DerivedFrom),
        _ => Err(QueryTextError::InvalidValue),
    }
}
```

- [ ] **Step 4: Verify GREEN**

Run:

```bash
cargo test -p continuitydb-query text_query_ --all-features
cargo test -p continuitydb-query --all-features
```

Expected: PASS.

- [ ] **Step 5: Commit parser implementation**

```bash
git add crates/continuitydb-query/src/lib.rs crates/continuitydb-query/src/text.rs docs/superpowers/plans/2026-05-20-text-query-dependency-constraints.md
git commit -m "feat: parse dependency text query constraints"
```

## Task 5: Documentation

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-text-query-dependency-constraints.md`

- [ ] **Step 1: Update README current scope**

Add this bullet after the bitemporal text query constraint bullet:

```markdown
- Dependency-aware constraints in strict text `CHECKOUT` queries.
```

- [ ] **Step 2: Update Query Language roadmap**

Add this milestone after the temporal and commit text query milestone:

```markdown
9. Add dependency constraints to text checkout queries. Implemented `dependency_target` and `dependency_kind` constraints, backed by `StateCellId` text parsing, so strict text `CHECKOUT` can express causality-aware materialization already available in the typed AST.
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

Expected: all commands exit successfully.

- [ ] **Step 4: Commit documentation**

```bash
git add README.md docs/roadmap.md docs/superpowers/plans/2026-05-20-text-query-dependency-constraints.md
git commit -m "docs: record text query dependency constraints"
```

## Self-Review

- Spec coverage: Tasks add `StateCellId` text round-trip, dependency target and kind parsing, invalid value handling, roadmap documentation, and verification.
- Placeholder scan: No placeholders or vague implementation steps remain.
- Type consistency: `StateCellId`, `CellDependencyKind`, `QueryRequirements.dependency_target`, and `dependency_kind` match existing crate types.
