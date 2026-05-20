# Checkout Query Text Parser Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the first strict text parser for `CHECKOUT` queries and compile parsed text into the existing `ContinuityQuery` AST.

**Architecture:** `continuitydb-query` gets a focused `text` module with a small lexer and parser. Public API is `parse_query_text(&str) -> Result<ContinuityQuery, QueryTextError>`, and execution remains unchanged because parsed text returns the existing AST.

**Tech Stack:** Rust 2021, `continuitydb-query`, existing `continuitydb-core` domain types, no new parser dependency for this first narrow grammar.

---

## File Structure

- Modify `crates/continuitydb-query/src/lib.rs`: expose the parser API, add parser tests, and add the module declaration.
- Create `crates/continuitydb-query/src/text.rs`: implement `QueryTextError`, lexer, parser, scope/value handling, and `parse_query_text`.
- Modify `README.md`: add first text query parsing to current scope.
- Modify `docs/roadmap.md`: add Query Language milestone.
- Modify this plan: mark steps complete as executed.

## Task 1: Failing Parser Tests

**Files:**
- Modify: `crates/continuitydb-query/src/lib.rs`

- [x] **Step 1: Add parser imports**

Update the test import block to use `parse_query_text` and `QueryTextError` through `super::*`. No additional external imports are needed.

- [x] **Step 2: Add parser behavior tests**

Add tests after `decoded_query_envelope_can_be_inspected_before_compile` for:

```rust
#[test]
fn text_query_parses_minimal_checkout() -> Result<(), Box<dyn std::error::Error>> {
    let query = parse_query_text(r#"CHECKOUT "release" ANSWER "what should ship?""#)?;

    let ContinuityQuery::Checkout(checkout) = query;
    assert_eq!(checkout.task().name, "release");
    assert_eq!(checkout.task().answerability_question, "what should ship?");
    assert_eq!(checkout.requirements(), &QueryRequirements::default());
    Ok(())
}

#[test]
fn text_query_parses_checkout_where_constraints() -> Result<(), Box<dyn std::error::Error>> {
    let query = parse_query_text(
        r#"CHECKOUT "release" ANSWER "what should ship?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
  AND token_budget <= 1200
  AND evidence_source = "source:release-notes""#,
    )?;

    let ContinuityQuery::Checkout(checkout) = query;
    assert_eq!(
        checkout.requirements().scope,
        Some(Scope::Project("continuitydb".to_string()))
    );
    assert_eq!(
        checkout.requirements().minimum_confidence,
        Confidence::new(0.7)?
    );
    assert_eq!(checkout.requirements().token_budget, 1200);
    assert_eq!(
        checkout.requirements().evidence_source.as_deref(),
        Some("source:release-notes")
    );
    Ok(())
}

#[test]
fn text_query_keywords_are_case_insensitive() -> Result<(), Box<dyn std::error::Error>> {
    let query = parse_query_text(r#"checkout "release" answer "what should ship?" where scope = global"#)?;

    let ContinuityQuery::Checkout(checkout) = query;
    assert_eq!(checkout.requirements().scope, Some(Scope::Global));
    Ok(())
}

#[test]
fn text_query_rejects_invalid_syntax() {
    assert_eq!(
        parse_query_text(r#"CHECKOUT "release" WHERE scope = global"#),
        Err(QueryTextError::InvalidSyntax)
    );
}

#[test]
fn text_query_rejects_invalid_values() {
    assert_eq!(
        parse_query_text(r#"CHECKOUT "release" ANSWER "what should ship?" WHERE min_confidence >= 1.5"#),
        Err(QueryTextError::InvalidValue)
    );
    assert_eq!(
        parse_query_text(r#"CHECKOUT "release" ANSWER "what should ship?" WHERE token_budget <= -1"#),
        Err(QueryTextError::InvalidValue)
    );
}
```

- [x] **Step 3: Verify RED**

Run:

```bash
cargo test -p continuitydb-query text_query --all-features
```

Expected: FAIL because `parse_query_text` and `QueryTextError` do not exist.

## Task 2: Parser Implementation

**Files:**
- Modify: `crates/continuitydb-query/src/lib.rs`
- Create: `crates/continuitydb-query/src/text.rs`

- [x] **Step 1: Add module and exports**

At the top of `lib.rs`, add:

```rust
mod text;

pub use text::{parse_query_text, QueryTextError};
```

- [x] **Step 2: Implement `text.rs`**

Create `crates/continuitydb-query/src/text.rs` with:

- `QueryTextError::{InvalidSyntax, InvalidValue}`;
- `Token` enum for identifiers, quoted strings, numbers, parentheses, and operators;
- `Lexer` that consumes the input without panics or unwraps;
- `Parser` that consumes `CHECKOUT string ANSWER string [WHERE constraint (AND constraint)*]`;
- scope constructors for `project`, `team`, `org`, `personal`, `task`, and `global`;
- bounded confidence and non-negative token budget validation.

- [x] **Step 3: Verify GREEN**

Run:

```bash
cargo test -p continuitydb-query text_query --all-features
cargo test -p continuitydb-query --all-features
```

Expected: PASS.

- [x] **Step 4: Commit parser implementation**

```bash
git add crates/continuitydb-query/src/lib.rs crates/continuitydb-query/src/text.rs docs/superpowers/plans/2026-05-20-checkout-query-text-parser.md
git commit -m "feat: parse checkout query text"
```

## Task 3: Docs and Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-checkout-query-text-parser.md`

- [ ] **Step 1: Update README**

Add this current-scope bullet near query bullets:

```markdown
- First strict text parser for `CHECKOUT` queries.
```

- [ ] **Step 2: Update roadmap**

Add this Query Language milestone after AST introspection:

```markdown
7. Add first strict text parser for checkout queries. Implemented `parse_query_text` for a minimal `CHECKOUT "task" ANSWER "question"` syntax with deterministic `WHERE` constraints for scope, minimum confidence, token budget, and evidence source.
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
git add README.md docs/roadmap.md docs/superpowers/plans/2026-05-20-checkout-query-text-parser.md
git commit -m "docs: record checkout query text parser"
```
