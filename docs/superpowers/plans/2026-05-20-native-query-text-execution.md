# Native Query Text Execution Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `ContinuityDb::checkout_query_text` so embedders can execute strict text `CHECKOUT` queries directly from in-memory strings.

**Architecture:** The native API parses text with `continuitydb_query::parse_query_text`, then delegates to the existing `checkout_continuity_query` path. This preserves the typed AST as the semantic boundary and keeps parser, compiler, and checkout errors distinct.

**Tech Stack:** Rust 2021, `continuitydb-api`, `continuitydb-query`, existing strict text parser and checkout execution.

---

## File Structure

- Modify `crates/continuitydb-api/src/lib.rs`: add `checkout_query_text` and native API tests.
- Modify `README.md`: add direct native text query execution to current scope.
- Modify `docs/roadmap.md`: add Native API milestone for direct strict text query execution.
- Modify this plan: mark steps complete as executed.

## Task 1: Failing Native API Tests

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`

- [x] **Step 1: Add direct text execution test**

Add this test after `api_checkout_query_json_reports_invalid_envelope`:

```rust
#[test]
fn api_checkout_query_text_materializes_slice() -> Result<(), Box<dyn std::error::Error>> {
    let mut db = ContinuityDb::new(MemoryKernel::default());
    db.ingest_cell(sample_cell("project:continuitydb:query-text", 0.91, 12)?)?;

    let slice = db.checkout_query_text(
        r#"CHECKOUT "stored-facts" ANSWER "what should the agent know?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
  AND token_budget <= 1200"#,
    )?;

    assert_eq!(slice.cells.len(), 1);
    assert_eq!(
        slice.cells[0].payload,
        CellPayload::Text("project:continuitydb:query-text".to_string())
    );
    Ok(())
}
```

- [x] **Step 2: Add direct text parser error test**

Add this test after `api_checkout_query_text_materializes_slice`:

```rust
#[test]
fn api_checkout_query_text_reports_invalid_text_query() {
    let db = ContinuityDb::new(MemoryKernel::default());

    let result = db.checkout_query_text(r#"CHECKOUT "stored-facts" WHERE scope = global"#);

    assert_eq!(
        result.err(),
        Some(ContinuityError::QueryText(QueryTextError::InvalidSyntax))
    );
}
```

- [x] **Step 3: Verify RED**

Run:

```bash
cargo test -p continuitydb-api checkout_query_text --all-features
```

Expected: FAIL because `ContinuityDb::checkout_query_text` does not exist.

## Task 2: Native API Implementation

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`

- [x] **Step 1: Add native method**

Add this method immediately after `checkout_query_json`:

```rust
/// Materializes a deterministic continuity slice from strict text query syntax.
pub fn checkout_query_text(&self, input: &str) -> Result<CheckoutSlice, ContinuityError> {
    self.checkout_continuity_query(parse_query_text(input)?)
}
```

- [x] **Step 2: Verify GREEN**

Run:

```bash
cargo test -p continuitydb-api checkout_query_text --all-features
cargo test -p continuitydb-api --all-features
```

Expected: PASS.

- [ ] **Step 3: Commit implementation**

```bash
git add crates/continuitydb-api/src/lib.rs docs/superpowers/plans/2026-05-20-native-query-text-execution.md
git commit -m "feat: execute text queries through native api"
```

## Task 3: Documentation

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-native-query-text-execution.md`

- [ ] **Step 1: Update README current scope**

Add this bullet after native saved text query-file execution:

```markdown
- Native API execution for strict text `CHECKOUT` query strings.
```

- [ ] **Step 2: Update Native API roadmap**

Add this milestone after native saved text query-file execution and renumber the later Native API milestones:

```markdown
6. Add native strict text query execution. Implemented `ContinuityDb::checkout_query_text` so embedders can execute strict `CHECKOUT` text directly without creating saved query files, preserving text parser and query compilation error boundaries.
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
git add README.md docs/roadmap.md docs/superpowers/plans/2026-05-20-native-query-text-execution.md
git commit -m "docs: record native query text execution"
```

## Self-Review

- Spec coverage: Tasks add the native method, parser-error tests, README current scope, and Native API roadmap milestone.
- Placeholder scan: No placeholders or deferred implementation steps remain.
- Type consistency: `checkout_query_text(&self, input: &str) -> Result<CheckoutSlice, ContinuityError>` matches the design and existing API style.
