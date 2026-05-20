# Text Query Temporal and Commit Constraints Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend strict text `CHECKOUT` queries with `valid_at`, `system_at`, and `commit_id` constraints.

**Architecture:** The parser keeps the existing strict lexer and treats temporal and commit values as quoted strings. Parser helpers convert RFC3339 timestamps into `DateTime<Utc>` and UUID commit text into `CommitId`, storing the results in `QueryRequirements` before normal checkout compilation.

**Tech Stack:** Rust 2021, `continuitydb-query`, `chrono`, existing `CommitId` parsing, existing text parser.

---

## File Structure

- Modify `crates/continuitydb-query/src/text.rs`: import `DateTime`, `Utc`, and `CommitId`; add parser branches and parse helpers.
- Modify `crates/continuitydb-query/src/lib.rs`: add parser tests in the existing query test module.
- Modify `README.md`: record temporal and commit text query constraints in current scope.
- Modify `docs/roadmap.md`: add Query Language milestone for text temporal and commit constraints.
- Modify this plan: mark steps complete as executed.

## Task 1: Failing Parser Tests

**Files:**
- Modify: `crates/continuitydb-query/src/lib.rs`

- [x] **Step 1: Add parser test for temporal and commit constraints**

Add this test after `text_query_parses_checkout_where_constraints`:

```rust
#[test]
fn text_query_parses_temporal_and_commit_constraints(
) -> Result<(), Box<dyn std::error::Error>> {
    let commit_id = CommitId::new();
    let query = parse_query_text(&format!(
        r#"CHECKOUT "release" ANSWER "what should ship?"
WHERE valid_at = "2026-05-20T12:00:00Z"
  AND system_at = "2026-05-20T12:30:00Z"
  AND commit_id = "{commit_id}""#
    ))?;

    let ContinuityQuery::Checkout(checkout) = query;
    assert_eq!(
        checkout.requirements().valid_at,
        Some(
            Utc.with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
                .single()
                .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?
        )
    );
    assert_eq!(
        checkout.requirements().system_at,
        Some(
            Utc.with_ymd_and_hms(2026, 5, 20, 12, 30, 0)
                .single()
                .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?
        )
    );
    assert_eq!(checkout.requirements().commit_id, Some(commit_id));
    Ok(())
}
```

- [x] **Step 2: Add invalid temporal and commit value tests**

Extend `text_query_rejects_invalid_values` with:

```rust
assert_eq!(
    parse_query_text(
        r#"CHECKOUT "release" ANSWER "what should ship?" WHERE valid_at = "not-a-time""#
    ),
    Err(QueryTextError::InvalidValue)
);
assert_eq!(
    parse_query_text(
        r#"CHECKOUT "release" ANSWER "what should ship?" WHERE commit_id = "not-a-uuid""#
    ),
    Err(QueryTextError::InvalidValue)
);
```

- [x] **Step 3: Verify RED**

Run:

```bash
cargo test -p continuitydb-query text_query_ --all-features
```

Expected: FAIL because `valid_at`, `system_at`, and `commit_id` are not supported text constraints.

## Task 2: Parser Implementation

**Files:**
- Modify: `crates/continuitydb-query/src/text.rs`

- [x] **Step 1: Add imports**

Change the imports at the top of `text.rs` to:

```rust
use chrono::{DateTime, Utc};
use continuitydb_core::{CommitId, Confidence, Scope};
```

- [x] **Step 2: Add constraint branches**

Add these branches to `parse_constraint` after `scope`:

```rust
"valid_at" => {
    self.expect_token(Token::Eq)?;
    requirements.valid_at = Some(self.parse_datetime()?);
}
"system_at" => {
    self.expect_token(Token::Eq)?;
    requirements.system_at = Some(self.parse_datetime()?);
}
"commit_id" => {
    self.expect_token(Token::Eq)?;
    requirements.commit_id = Some(self.parse_commit_id()?);
}
```

- [x] **Step 3: Add parse helpers**

Add these methods before `parse_scope`:

```rust
fn parse_datetime(&mut self) -> Result<DateTime<Utc>, QueryTextError> {
    DateTime::parse_from_rfc3339(&self.expect_string()?)
        .map(|value| value.with_timezone(&Utc))
        .map_err(|_error| QueryTextError::InvalidValue)
}

fn parse_commit_id(&mut self) -> Result<CommitId, QueryTextError> {
    self.expect_string()?
        .parse::<CommitId>()
        .map_err(|_error| QueryTextError::InvalidValue)
}
```

- [x] **Step 4: Verify GREEN**

Run:

```bash
cargo test -p continuitydb-query text_query_ --all-features
cargo test -p continuitydb-query --all-features
```

Expected: PASS.

- [x] **Step 5: Commit parser implementation**

```bash
git add crates/continuitydb-query/src/lib.rs crates/continuitydb-query/src/text.rs docs/superpowers/plans/2026-05-20-text-query-temporal-commit-constraints.md
git commit -m "feat: parse temporal text query constraints"
```

## Task 3: Documentation

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-text-query-temporal-commit-constraints.md`

- [x] **Step 1: Update README current scope**

Add this bullet after the first strict text parser bullet:

```markdown
- Bitemporal and commit-scoped constraints in strict text `CHECKOUT` queries.
```

- [x] **Step 2: Update Query Language roadmap**

Add this milestone after the first strict text parser milestone:

```markdown
8. Add temporal and commit constraints to text checkout queries. Implemented strict `valid_at`, `system_at`, and `commit_id` constraints so text `CHECKOUT` syntax can express bitemporal and commit-scoped materialization already available in the typed AST.
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

Expected: all commands exit successfully.

- [x] **Step 4: Commit documentation**

```bash
git add README.md docs/roadmap.md docs/superpowers/plans/2026-05-20-text-query-temporal-commit-constraints.md
git commit -m "docs: record text query temporal constraints"
```

## Self-Review

- Spec coverage: Tasks add temporal/commit syntax, invalid value handling, roadmap documentation, and verification.
- Placeholder scan: No placeholders or vague implementation steps remain.
- Type consistency: `DateTime<Utc>`, `CommitId`, `QueryRequirements.valid_at`, `system_at`, and `commit_id` match existing crate types.
