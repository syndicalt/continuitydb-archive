# Native Query Envelope Execution Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a native API helper that executes versioned typed query JSON envelope bytes.

**Architecture:** `continuitydb-api` delegates query envelope decoding to `continuitydb-query::decode_query_json`, maps envelope decode errors into a distinct native error variant, and executes the decoded `ContinuityQuery` through the existing typed query execution path.

**Tech Stack:** Rust 2021, `continuitydb-api`, `continuitydb-query`, `continuitydb-memory`.

---

## File Structure

- Modify `crates/continuitydb-api/src/lib.rs`: import query envelope helpers/errors, add a `ContinuityError` variant, add `checkout_query_json`, and add tests.
- Modify `README.md`: add native query-envelope execution to current scope.
- Modify `docs/roadmap.md`: add Native API milestone.
- Modify this plan: mark steps complete as executed.

## Task 1: Failing Native API Tests

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`

- [x] **Step 1: Add test imports**

Update the existing query import in `crates/continuitydb-api/src/lib.rs` to include envelope helpers:

```rust
use continuitydb_query::{
    encode_query_json, CheckoutQuery, ContinuityQuery, QueryEnvelope, QueryEnvelopeError,
    QueryError, QueryReturnShape, QueryTask, QUERY_ENVELOPE_FORMAT, QUERY_ENVELOPE_FORMAT_VERSION,
};
```

- [x] **Step 2: Add successful envelope execution test**

Add this test near the existing typed query API tests:

```rust
#[test]
fn api_checkout_query_json_materializes_slice() -> Result<(), Box<dyn std::error::Error>> {
    let mut db = ContinuityDb::new(MemoryKernel::default());
    db.ingest_cell(test_cell("project:continuitydb:query-json")?)?;
    let query = ContinuityQuery::Checkout(CheckoutQuery::new(QueryTask::new(
        "stored-facts",
        "what is stored?",
    )));
    let encoded = encode_query_json(query)?;

    let slice = db.checkout_query_json(&encoded)?;

    assert_eq!(slice.cells.len(), 1);
    assert_eq!(
        slice.cells[0].payload,
        continuitydb_core::CellPayload::Text("project:continuitydb:query-json".to_string())
    );
    Ok(())
}
```

- [x] **Step 3: Add unsupported query semantics test**

Add this test after the successful envelope test:

```rust
#[test]
fn api_checkout_query_json_preserves_query_compilation_errors(
) -> Result<(), Box<dyn std::error::Error>> {
    let db = ContinuityDb::new(MemoryKernel::default());
    let query = ContinuityQuery::Checkout(
        CheckoutQuery::new(QueryTask::new("stored-facts", "what is stored?"))
            .with_return_shape(QueryReturnShape::CellsOnly),
    );
    let encoded = encode_query_json(query)?;

    let result = db.checkout_query_json(&encoded);

    assert_eq!(
        result.err(),
        Some(ContinuityError::Query(QueryError::UnsupportedReturnShape(
            QueryReturnShape::CellsOnly
        )))
    );
    Ok(())
}
```

- [x] **Step 4: Add malformed JSON test**

Add this test after the unsupported query semantics test:

```rust
#[test]
fn api_checkout_query_json_reports_invalid_json() {
    let db = ContinuityDb::new(MemoryKernel::default());

    let result = db.checkout_query_json(b"{not valid json}\n");

    assert_eq!(
        result.err(),
        Some(ContinuityError::QueryEnvelope(
            QueryEnvelopeError::InvalidJson
        ))
    );
}
```

- [x] **Step 5: Add invalid envelope test**

Add this test after the malformed JSON test:

```rust
#[test]
fn api_checkout_query_json_reports_invalid_envelope(
) -> Result<(), Box<dyn std::error::Error>> {
    let db = ContinuityDb::new(MemoryKernel::default());
    let envelope = QueryEnvelope {
        format: QUERY_ENVELOPE_FORMAT.to_string(),
        version: QUERY_ENVELOPE_FORMAT_VERSION + 1,
        query: ContinuityQuery::Checkout(CheckoutQuery::new(QueryTask::new(
            "stored-facts",
            "what is stored?",
        ))),
    };
    let encoded = serde_json::to_vec(&envelope)?;

    let result = db.checkout_query_json(&encoded);

    assert_eq!(
        result.err(),
        Some(ContinuityError::QueryEnvelope(
            QueryEnvelopeError::InvalidEnvelope
        ))
    );
    Ok(())
}
```

- [x] **Step 6: Verify RED**

Run:

```bash
cargo test -p continuitydb-api checkout_query_json --all-features
```

Expected: FAIL because `checkout_query_json` and `ContinuityError::QueryEnvelope` do not exist.

## Task 2: Native API Implementation

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`

- [x] **Step 1: Add production imports**

Update the production query import near the top of `crates/continuitydb-api/src/lib.rs` to:

```rust
use continuitydb_query::{decode_query_json, CheckoutQuery, ContinuityQuery, QueryEnvelopeError, QueryError};
```

If test-only imports are needed, keep them inside the test module instead of production imports.

- [x] **Step 2: Add native error variant**

Add this variant to `ContinuityError` after `Query`:

```rust
    /// Query envelope decoding or validation failure.
    #[error(transparent)]
    QueryEnvelope(#[from] QueryEnvelopeError),
```

- [x] **Step 3: Add execution helper**

Add this method near the existing typed query execution methods in `impl<K: StorageKernel> ContinuityDb<K>`:

```rust
    /// Materializes a deterministic continuity slice from a versioned typed query JSON envelope.
    pub fn checkout_query_json(&self, bytes: &[u8]) -> Result<CheckoutSlice, ContinuityError> {
        self.checkout_continuity_query(decode_query_json(bytes)?)
    }
```

- [x] **Step 4: Verify GREEN**

Run:

```bash
cargo test -p continuitydb-api checkout_query_json --all-features
cargo test -p continuitydb-api --all-features
```

Expected: PASS.

- [x] **Step 5: Commit implementation**

```bash
git add crates/continuitydb-api/src/lib.rs docs/superpowers/plans/2026-05-20-native-query-envelope-execution.md
git commit -m "feat: execute query envelopes through native api"
```

## Task 3: Docs and Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-native-query-envelope-execution.md`

- [ ] **Step 1: Update README**

Add this current-scope bullet near the existing native query bullets:

```markdown
- Native API execution for versioned typed query envelopes.
```

- [ ] **Step 2: Update roadmap**

Add this Native API milestone after native typed query execution:

```markdown
3. Add native versioned query-envelope execution. Implemented `ContinuityDb::checkout_query_json` so embedders can execute `continuitydb.query` envelope bytes directly while preserving distinct query compilation and envelope validation errors.
```

Renumber the following Native API milestones.

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
git add README.md docs/roadmap.md docs/superpowers/plans/2026-05-20-native-query-envelope-execution.md
git commit -m "docs: record native query envelope execution"
```
