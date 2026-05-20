# Text Query File Execution Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Execute saved strict text `CHECKOUT` query files through the native API and existing CLI `checkout-query` command.

**Architecture:** `continuitydb-api` extends its saved query-file decoder to detect intentional text query files and call `continuitydb_query::parse_query_text`. The CLI remains thin because it already delegates `checkout-query` to `ContinuityDb::checkout_query_file`.

**Tech Stack:** Rust 2021, `continuitydb-api`, `continuitydb-query`, `continuitydb-cli`, existing query parser and checkout execution.

---

## File Structure

- Modify `crates/continuitydb-api/src/lib.rs`: import text parser types, add native text-query error variant, extend query-file detection, and add native tests.
- Modify `crates/continuitydb-cli/tests/cli.rs`: add end-to-end text query file test.
- Modify `README.md`: add text query file execution to current scope.
- Modify `docs/roadmap.md`: update Query Language, CLI, and Native API milestones.
- Modify this plan: mark steps complete as executed.

## Task 1: Failing Native API Tests

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`

- [x] **Step 1: Add test imports**

Update the test query import in `crates/continuitydb-api/src/lib.rs` to include `QueryTextError`:

```rust
use continuitydb_query::{
    encode_query_json, CheckoutQuery, ContinuityQuery, QueryEnvelope, QueryEnvelopeError,
    QueryError, QueryOptimization, QueryRequirements, QueryReturnShape, QueryTask,
    QueryTextError, QUERY_ENVELOPE_FORMAT, QUERY_ENVELOPE_FORMAT_VERSION,
};
```

- [x] **Step 2: Add native text query file tests**

Add these tests after `api_checkout_query_file_executes_raw_query_json`:

```rust
#[test]
fn api_checkout_query_file_executes_text_query() -> Result<(), Box<dyn std::error::Error>> {
    let mut db = ContinuityDb::new(MemoryKernel::default());
    db.ingest_cell(sample_cell("project:continuitydb:query-file-text", 0.91, 12)?)?;
    let path = temp_file_kernel_path("query-file-text");
    fs::write(
        &path,
        r#"CHECKOUT "stored-facts" ANSWER "what should the agent know?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
  AND token_budget <= 1200"#,
    )?;

    let slice = db.checkout_query_file(&path)?;

    assert_eq!(slice.cells.len(), 1);
    assert_eq!(
        slice.cells[0].payload,
        CellPayload::Text("project:continuitydb:query-file-text".to_string())
    );
    fs::remove_file(path)?;
    Ok(())
}

#[test]
fn api_checkout_query_file_reports_invalid_text_query(
) -> Result<(), Box<dyn std::error::Error>> {
    let db = ContinuityDb::new(MemoryKernel::default());
    let path = temp_file_kernel_path("invalid-query-file-text");
    fs::write(&path, r#"CHECKOUT "stored-facts" WHERE scope = global"#)?;

    let result = db.checkout_query_file(&path);

    assert_eq!(
        result.err(),
        Some(ContinuityError::QueryText(QueryTextError::InvalidSyntax))
    );
    fs::remove_file(path)?;
    Ok(())
}
```

- [x] **Step 3: Verify RED**

Run:

```bash
cargo test -p continuitydb-api checkout_query_file --all-features
```

Expected: FAIL because `ContinuityError::QueryText` does not exist and text files are not decoded.

## Task 2: Native API Implementation

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`

- [x] **Step 1: Add production imports**

Update the production query import near the top of `crates/continuitydb-api/src/lib.rs`:

```rust
use continuitydb_query::{
    decode_query_json, parse_query_text, CheckoutQuery, ContinuityQuery, QueryEnvelopeError,
    QueryError, QueryTextError,
};
```

- [x] **Step 2: Add native error variant**

Add this variant after `QueryEnvelope`:

```rust
    /// Query text parsing failure.
    #[error(transparent)]
    QueryText(#[from] QueryTextError),
```

- [x] **Step 3: Extend file decoder**

Update `decode_query_file` and add helper:

```rust
fn decode_query_file(bytes: &[u8]) -> Result<ContinuityQuery, ContinuityError> {
    if let Some(text) = query_text_input(bytes) {
        return parse_query_text(text).map_err(Into::into);
    }
    if is_query_envelope_shape(bytes)? {
        return decode_query_json(bytes).map_err(Into::into);
    }
    serde_json::from_slice::<ContinuityQuery>(bytes).map_err(|_error| ContinuityError::QueryJson)
}

fn query_text_input(bytes: &[u8]) -> Option<&str> {
    let text = std::str::from_utf8(bytes).ok()?;
    let trimmed = text.trim_start();
    if trimmed.len() >= "checkout".len()
        && trimmed[.."checkout".len()].eq_ignore_ascii_case("checkout")
    {
        Some(text)
    } else {
        None
    }
}
```

- [x] **Step 4: Verify GREEN**

Run:

```bash
cargo test -p continuitydb-api checkout_query_file --all-features
cargo test -p continuitydb-api --all-features
```

Expected: PASS.

- [x] **Step 5: Commit native implementation**

```bash
git add crates/continuitydb-api/src/lib.rs docs/superpowers/plans/2026-05-20-text-query-file-execution.md
git commit -m "feat: execute text query files through native api"
```

## Task 3: CLI Text Query File Test

**Files:**
- Modify: `crates/continuitydb-cli/tests/cli.rs`

- [ ] **Step 1: Add CLI text query file test**

Add this test after `cli_checkout_query_executes_versioned_query_envelope`:

```rust
#[test]
fn cli_checkout_query_executes_text_query_file() -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path("continuitydb-cli-checkout-query-text-store");
    let query_path = temp_store_path("continuitydb-cli-checkout-query-text-query");
    write_committed_store(&store_path, "project:continuitydb:cli-query-text")?;
    fs::write(
        &query_path,
        r#"CHECKOUT "stored-facts" ANSWER "what is stored?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
  AND token_budget <= 1200"#,
    )?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(json["cells"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        json["cells"][0]["payload"]["Text"].as_str(),
        Some("project:continuitydb:cli-query-text")
    );

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    Ok(())
}
```

- [ ] **Step 2: Verify CLI GREEN**

Run:

```bash
cargo test -p continuitydb-cli checkout_query --all-features
```

Expected: PASS.

- [ ] **Step 3: Commit CLI test**

```bash
git add crates/continuitydb-cli/tests/cli.rs docs/superpowers/plans/2026-05-20-text-query-file-execution.md
git commit -m "test: cover cli text query files"
```

## Task 4: Docs and Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-text-query-file-execution.md`

- [ ] **Step 1: Update README**

Add this current-scope bullet near query bullets:

```markdown
- Native and CLI execution for saved text `CHECKOUT` query files.
```

- [ ] **Step 2: Update roadmap**

Add this Native API milestone after saved query-file execution:

```markdown
5. Add native saved text query-file execution. Extended `ContinuityDb::checkout_query_file` to recognize strict text `CHECKOUT` query files and route them through `parse_query_text` before normal typed query execution.
```

Renumber following Native API milestones.

Update CLI milestone 14 to:

```markdown
14. Execute saved query files from the CLI. Extended `continuitydb checkout-query` to accept raw typed query JSON, versioned `continuitydb.query` envelopes, and strict text `CHECKOUT` query files through the native query-file API.
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
git add README.md docs/roadmap.md docs/superpowers/plans/2026-05-20-text-query-file-execution.md
git commit -m "docs: record text query file execution"
```
