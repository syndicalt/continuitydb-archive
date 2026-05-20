# Query Envelope Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a versioned JSON envelope for serialized typed Continuity queries.

**Architecture:** `continuitydb-query` owns query wire-format metadata through a small `QueryEnvelope` type plus encode/decode helpers. Raw AST serde remains available, while the new helpers validate format and version before returning a `ContinuityQuery`.

**Tech Stack:** Rust 2021, `continuitydb-query`, `serde`, `serde_json`, `thiserror`.

---

## File Structure

- Modify `crates/continuitydb-query/Cargo.toml`: promote `serde_json` from dev dependency to production dependency for envelope helpers.
- Modify `crates/continuitydb-query/src/lib.rs`: add envelope constants, `QueryEnvelope`, `QueryEnvelopeError`, encode/decode helpers, and tests.
- Modify `README.md`: add versioned query envelope support to current scope.
- Modify `docs/roadmap.md`: add Query Language milestone.
- Modify this plan: mark steps complete as executed.

## Task 1: Failing Envelope Tests

**Files:**
- Modify: `crates/continuitydb-query/src/lib.rs`

- [x] **Step 1: Add envelope tests**

Add these tests to the existing test module in `crates/continuitydb-query/src/lib.rs`:

```rust
#[test]
fn query_envelope_encodes_format_version_and_query() -> Result<(), Box<dyn std::error::Error>> {
    let query = ContinuityQuery::Checkout(CheckoutQuery::new(QueryTask::new(
        "stored-facts",
        "what is stored?",
    )));

    let encoded = encode_query_json(query.clone())?;
    let value: serde_json::Value = serde_json::from_slice(&encoded)?;

    assert_eq!(value["format"].as_str(), Some(QUERY_ENVELOPE_FORMAT));
    assert_eq!(
        value["version"].as_u64(),
        Some(QUERY_ENVELOPE_FORMAT_VERSION as u64)
    );
    assert!(value["query"]["checkout"].is_object());
    assert_eq!(decode_query_json(&encoded)?, query);
    Ok(())
}

#[test]
fn query_envelope_rejects_unsupported_format() -> Result<(), Box<dyn std::error::Error>> {
    let envelope = QueryEnvelope {
        format: "continuitydb.other".to_string(),
        version: QUERY_ENVELOPE_FORMAT_VERSION,
        query: ContinuityQuery::Checkout(CheckoutQuery::new(QueryTask::new(
            "stored-facts",
            "what is stored?",
        ))),
    };
    let encoded = serde_json::to_vec(&envelope)?;

    assert_eq!(
        decode_query_json(&encoded),
        Err(QueryEnvelopeError::InvalidEnvelope)
    );
    Ok(())
}

#[test]
fn query_envelope_rejects_unsupported_version() -> Result<(), Box<dyn std::error::Error>> {
    let envelope = QueryEnvelope {
        format: QUERY_ENVELOPE_FORMAT.to_string(),
        version: QUERY_ENVELOPE_FORMAT_VERSION + 1,
        query: ContinuityQuery::Checkout(CheckoutQuery::new(QueryTask::new(
            "stored-facts",
            "what is stored?",
        ))),
    };
    let encoded = serde_json::to_vec(&envelope)?;

    assert_eq!(
        decode_query_json(&encoded),
        Err(QueryEnvelopeError::InvalidEnvelope)
    );
    Ok(())
}

#[test]
fn query_envelope_rejects_malformed_json() {
    assert_eq!(
        decode_query_json(b"{not valid json}\n"),
        Err(QueryEnvelopeError::InvalidJson)
    );
}
```

- [x] **Step 2: Verify RED**

Run:

```bash
cargo test -p continuitydb-query query_envelope --all-features
```

Expected: FAIL because `QueryEnvelope`, `QueryEnvelopeError`, constants, and helpers are not implemented.

## Task 2: Envelope Implementation

**Files:**
- Modify: `crates/continuitydb-query/Cargo.toml`
- Modify: `crates/continuitydb-query/src/lib.rs`

- [x] **Step 1: Promote serde_json dependency**

Move `serde_json.workspace = true` from `[dev-dependencies]` to `[dependencies]` in `crates/continuitydb-query/Cargo.toml`:

```toml
[dependencies]
chrono.workspace = true
continuitydb-checkout = { path = "../continuitydb-checkout" }
continuitydb-core = { path = "../continuitydb-core" }
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
```

Remove the empty `[dev-dependencies]` section if no dev dependencies remain.

- [x] **Step 2: Add envelope constants and type**

Add this near the top-level query type definitions in `crates/continuitydb-query/src/lib.rs`:

```rust
/// Wire-format marker for versioned query JSON envelopes.
pub const QUERY_ENVELOPE_FORMAT: &str = "continuitydb.query";
/// Supported query JSON envelope version.
pub const QUERY_ENVELOPE_FORMAT_VERSION: u32 = 1;

/// Versioned JSON envelope for portable typed query files.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QueryEnvelope {
    /// Wire-format marker.
    pub format: String,
    /// Wire-format version.
    pub version: u32,
    /// Serialized typed query.
    pub query: ContinuityQuery,
}

impl QueryEnvelope {
    /// Wraps a typed query in the current JSON envelope.
    pub fn new(query: ContinuityQuery) -> Self {
        Self {
            format: QUERY_ENVELOPE_FORMAT.to_string(),
            version: QUERY_ENVELOPE_FORMAT_VERSION,
            query,
        }
    }

    /// Validates the envelope format and version.
    pub fn validate(&self) -> Result<(), QueryEnvelopeError> {
        if self.format == QUERY_ENVELOPE_FORMAT && self.version == QUERY_ENVELOPE_FORMAT_VERSION {
            Ok(())
        } else {
            Err(QueryEnvelopeError::InvalidEnvelope)
        }
    }
}
```

- [x] **Step 3: Add encode/decode helpers and error**

Add this after the `QueryError` definition:

```rust
/// Query envelope encoding and decoding errors.
#[derive(Debug, Error, PartialEq)]
pub enum QueryEnvelopeError {
    /// Query envelope JSON could not be encoded or decoded.
    #[error("query envelope JSON is invalid")]
    InvalidJson,
    /// Query envelope has an unsupported format or version.
    #[error("query envelope is invalid")]
    InvalidEnvelope,
}

/// Encodes a typed query as a versioned JSON envelope.
pub fn encode_query_json(query: ContinuityQuery) -> Result<Vec<u8>, QueryEnvelopeError> {
    serde_json::to_vec(&QueryEnvelope::new(query)).map_err(|_error| QueryEnvelopeError::InvalidJson)
}

/// Decodes a typed query from a versioned JSON envelope.
pub fn decode_query_json(bytes: &[u8]) -> Result<ContinuityQuery, QueryEnvelopeError> {
    let envelope = serde_json::from_slice::<QueryEnvelope>(bytes)
        .map_err(|_error| QueryEnvelopeError::InvalidJson)?;
    envelope.validate()?;
    Ok(envelope.query)
}
```

- [x] **Step 4: Verify GREEN**

Run:

```bash
cargo test -p continuitydb-query query_envelope --all-features
cargo test -p continuitydb-query --all-features
```

Expected: PASS.

- [x] **Step 5: Commit implementation**

```bash
git add crates/continuitydb-query/Cargo.toml crates/continuitydb-query/src/lib.rs Cargo.lock docs/superpowers/plans/2026-05-20-query-envelope.md
git commit -m "feat: add query json envelope"
```

## Task 3: Docs and Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-query-envelope.md`

- [ ] **Step 1: Update README**

Add this current-scope bullet near the existing query bullets:

```markdown
- Versioned JSON envelopes for portable typed query files.
```

- [ ] **Step 2: Update roadmap**

Add this Query Language milestone after CLI query-file execution:

```markdown
5. Add a versioned typed query JSON envelope. Implemented `QueryEnvelope`, `encode_query_json`, and `decode_query_json` in `continuitydb-query` so saved query files and bindings can validate format and version before executing raw typed query content.
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
git add README.md docs/roadmap.md docs/superpowers/plans/2026-05-20-query-envelope.md
git commit -m "docs: record query envelope"
```
