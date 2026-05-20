# Native Query File Execution Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a native API helper that executes saved typed query files, preserving CLI compatibility with raw `ContinuityQuery` JSON and versioned `continuitydb.query` envelopes.

**Architecture:** `continuitydb-api` owns query-file orchestration for any `StorageKernel`: read bytes, detect envelope-shaped JSON, decode either through `decode_query_json` or raw `ContinuityQuery` serde, then execute via `checkout_continuity_query`. The CLI delegates `checkout-query` to this native helper.

**Tech Stack:** Rust 2021, `continuitydb-api`, `continuitydb-query`, `continuitydb-memory`, `continuitydb-cli`, serde JSON.

---

## File Structure

- Modify `crates/continuitydb-api/src/lib.rs`: add native query file errors, `checkout_query_file`, private decode helpers, and native API tests.
- Modify `crates/continuitydb-cli/src/main.rs`: remove CLI-owned query-file decoding and route `checkout-query` through `ContinuityDb::checkout_query_file`.
- Modify `README.md`: add native query-file execution to current scope.
- Modify `docs/roadmap.md`: add Native API milestone and update CLI milestone.
- Modify this plan: mark steps complete as executed.

## Task 1: Failing Native API Tests

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`

- [x] **Step 1: Add query-file success tests**

Add these tests after `api_checkout_query_json_reports_invalid_envelope`:

```rust
#[test]
fn api_checkout_query_file_executes_query_envelope() -> Result<(), Box<dyn std::error::Error>> {
    let mut db = ContinuityDb::new(MemoryKernel::default());
    db.ingest_cell(sample_cell("project:continuitydb:query-file-envelope", 0.91, 12)?)?;
    let query = ContinuityQuery::Checkout(CheckoutQuery::new(QueryTask::new(
        "stored-facts",
        "what should the agent know?",
    )));
    let path = temp_file_kernel_path("query-file-envelope");
    fs::write(&path, encode_query_json(query)?)?;

    let slice = db.checkout_query_file(&path)?;

    assert_eq!(slice.cells.len(), 1);
    assert_eq!(
        slice.cells[0].payload,
        CellPayload::Text("project:continuitydb:query-file-envelope".to_string())
    );
    fs::remove_file(path)?;
    Ok(())
}

#[test]
fn api_checkout_query_file_executes_raw_query_json() -> Result<(), Box<dyn std::error::Error>> {
    let mut db = ContinuityDb::new(MemoryKernel::default());
    db.ingest_cell(sample_cell("project:continuitydb:query-file-raw", 0.91, 12)?)?;
    let query = ContinuityQuery::Checkout(CheckoutQuery::new(QueryTask::new(
        "stored-facts",
        "what should the agent know?",
    )));
    let path = temp_file_kernel_path("query-file-raw");
    fs::write(&path, serde_json::to_vec(&query)?)?;

    let slice = db.checkout_query_file(&path)?;

    assert_eq!(slice.cells.len(), 1);
    assert_eq!(
        slice.cells[0].payload,
        CellPayload::Text("project:continuitydb:query-file-raw".to_string())
    );
    fs::remove_file(path)?;
    Ok(())
}
```

- [x] **Step 2: Add query-file error tests**

Add these tests after the success tests:

```rust
#[test]
fn api_checkout_query_file_reports_missing_file() {
    let db = ContinuityDb::new(MemoryKernel::default());
    let path = temp_file_kernel_path("missing-query-file");

    let result = db.checkout_query_file(&path);

    assert_eq!(result.err(), Some(ContinuityError::QueryFileIo));
}

#[test]
fn api_checkout_query_file_reports_invalid_raw_json() -> Result<(), Box<dyn std::error::Error>> {
    let db = ContinuityDb::new(MemoryKernel::default());
    let path = temp_file_kernel_path("invalid-query-file-json");
    fs::write(&path, b"{not valid json}\n")?;

    let result = db.checkout_query_file(&path);

    assert_eq!(result.err(), Some(ContinuityError::QueryJson));
    fs::remove_file(path)?;
    Ok(())
}

#[test]
fn api_checkout_query_file_reports_invalid_envelope() -> Result<(), Box<dyn std::error::Error>> {
    let db = ContinuityDb::new(MemoryKernel::default());
    let envelope = QueryEnvelope {
        format: QUERY_ENVELOPE_FORMAT.to_string(),
        version: QUERY_ENVELOPE_FORMAT_VERSION + 1,
        query: ContinuityQuery::Checkout(CheckoutQuery::new(QueryTask::new(
            "stored-facts",
            "what should the agent know?",
        ))),
    };
    let path = temp_file_kernel_path("invalid-query-file-envelope");
    fs::write(&path, serde_json::to_vec(&envelope)?)?;

    let result = db.checkout_query_file(&path);

    assert_eq!(
        result.err(),
        Some(ContinuityError::QueryEnvelope(
            QueryEnvelopeError::InvalidEnvelope
        ))
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

Expected: FAIL because `checkout_query_file`, `ContinuityError::QueryFileIo`, and `ContinuityError::QueryJson` do not exist.

## Task 2: Native API Implementation

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`

- [x] **Step 1: Add native error variants**

Add these variants after `QueryEnvelope` in `ContinuityError`:

```rust
    /// Raw typed query JSON could not be decoded.
    #[error("query JSON is invalid")]
    QueryJson,
    /// Query file could not be read.
    #[error("query file I/O failed")]
    QueryFileIo,
```

- [x] **Step 2: Add file execution helper**

Add this method after `checkout_query_json`:

```rust
    /// Materializes a deterministic continuity slice from a saved typed query file.
    pub fn checkout_query_file<P: AsRef<Path>>(
        &self,
        query_path: P,
    ) -> Result<CheckoutSlice, ContinuityError> {
        let encoded = fs::read(query_path).map_err(|_error| ContinuityError::QueryFileIo)?;
        self.checkout_continuity_query(decode_query_file(&encoded)?)
    }
```

- [x] **Step 3: Add private query-file decode helpers**

Add these private helpers after the `impl<K: StorageKernel> ContinuityDb<K>` block:

```rust
fn decode_query_file(bytes: &[u8]) -> Result<ContinuityQuery, ContinuityError> {
    if is_query_envelope_shape(bytes)? {
        return decode_query_json(bytes).map_err(Into::into);
    }
    serde_json::from_slice::<ContinuityQuery>(bytes).map_err(|_error| ContinuityError::QueryJson)
}

fn is_query_envelope_shape(bytes: &[u8]) -> Result<bool, ContinuityError> {
    let value = serde_json::from_slice::<serde_json::Value>(bytes)
        .map_err(|_error| ContinuityError::QueryJson)?;
    Ok(value.get("format").is_some()
        && value.get("version").is_some()
        && value.get("query").is_some())
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
git add crates/continuitydb-api/src/lib.rs docs/superpowers/plans/2026-05-20-native-query-file-execution.md
git commit -m "feat: execute query files through native api"
```

## Task 3: CLI Refactor

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`

- [x] **Step 1: Route checkout-query through native helper**

Replace `checkout_query_file` with:

```rust
fn checkout_query_file(
    store_path: &PathBuf,
    query_path: &PathBuf,
) -> Result<continuitydb_checkout::CheckoutSlice, Box<dyn std::error::Error>> {
    let db = open_file_database(store_path)?;
    db.checkout_query_file(query_path).map_err(Into::into)
}
```

- [x] **Step 2: Remove CLI query decode imports**

Change:

```rust
use continuitydb_query::{decode_query_json, ContinuityQuery};
```

to:

```rust
use continuitydb_query::ContinuityQuery;
```

Delete the now-unused private `decode_query_file` and `is_query_envelope_shape` functions.

- [x] **Step 3: Verify CLI compatibility**

Run:

```bash
cargo test -p continuitydb-cli checkout_query --all-features
```

Expected: PASS, including raw query JSON and versioned query envelope tests.

- [x] **Step 4: Commit CLI refactor**

```bash
git add crates/continuitydb-cli/src/main.rs docs/superpowers/plans/2026-05-20-native-query-file-execution.md
git commit -m "refactor: route cli query files through native api"
```

## Task 4: Docs and Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-native-query-file-execution.md`

- [ ] **Step 1: Update README**

Add this current-scope bullet near the native query execution bullets:

```markdown
- Native API execution for saved typed query files.
```

- [ ] **Step 2: Update roadmap**

Add this Native API milestone after native versioned query-envelope execution:

```markdown
4. Add native saved query-file execution. Implemented `ContinuityDb::checkout_query_file` so embedders can execute either raw `ContinuityQuery` JSON files or versioned `continuitydb.query` envelope files while preserving native query, envelope, JSON, and file I/O error boundaries.
```

Renumber the following Native API milestones.

Update the CLI query-envelope milestone to mention native routing:

```markdown
14. Execute versioned typed query envelopes from the CLI. Extended `continuitydb checkout-query` to accept `continuitydb.query` envelope files while preserving raw typed query JSON compatibility, routed through the native query-file API.
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
git add README.md docs/roadmap.md docs/superpowers/plans/2026-05-20-native-query-file-execution.md
git commit -m "docs: record native query file execution"
```
