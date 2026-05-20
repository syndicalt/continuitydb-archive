# CLI Query Envelope Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let `continuitydb checkout-query` execute versioned query-envelope JSON files while preserving raw AST query compatibility.

**Architecture:** The CLI reads query file bytes and routes decoding through a small helper. Envelope-shaped objects are decoded through `continuitydb_query::decode_query_json`; other JSON is decoded as raw `ContinuityQuery` for compatibility.

**Tech Stack:** Rust 2021, `continuitydb-cli`, `continuitydb-query`, `serde_json`, `assert_cmd`.

---

## File Structure

- Modify `crates/continuitydb-cli/src/main.rs`: import `decode_query_json` and add a decode helper that distinguishes envelope-shaped files from raw AST files.
- Modify `crates/continuitydb-cli/tests/cli.rs`: add CLI tests for envelope execution and invalid envelope rejection.
- Modify `README.md`: mention CLI support for versioned query envelopes.
- Modify `docs/roadmap.md`: add a CLI milestone for query-envelope execution.
- Modify this plan: mark steps complete as executed.

## Task 1: Failing CLI Envelope Tests

**Files:**
- Modify: `crates/continuitydb-cli/tests/cli.rs`

- [ ] **Step 1: Add query envelope test imports**

Update the existing `continuitydb_query` import in `crates/continuitydb-cli/tests/cli.rs` to include `QueryEnvelope`, `QUERY_ENVELOPE_FORMAT`, `QUERY_ENVELOPE_FORMAT_VERSION`, and `encode_query_json`:

```rust
use continuitydb_query::{
    encode_query_json, CheckoutQuery, ContinuityQuery, QueryEnvelope, QueryOptimization,
    QueryRequirements, QueryReturnShape, QueryTask, QUERY_ENVELOPE_FORMAT,
    QUERY_ENVELOPE_FORMAT_VERSION,
};
```

- [ ] **Step 2: Add successful envelope checkout test**

Add this test after `cli_checkout_query_executes_serialized_typed_query`:

```rust
#[test]
fn cli_checkout_query_executes_versioned_query_envelope(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path("continuitydb-cli-checkout-query-envelope-store");
    let query_path = temp_store_path("continuitydb-cli-checkout-query-envelope-query");
    write_committed_store(&store_path, "project:continuitydb:cli-query-envelope")?;
    let query = ContinuityQuery::Checkout(
        CheckoutQuery::new(QueryTask::new("stored-facts", "what is stored?")).with_requirements(
            QueryRequirements {
                scope: Some(Scope::Project("continuitydb".to_string())),
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 1200,
                ..QueryRequirements::default()
            },
        ),
    );
    fs::write(&query_path, encode_query_json(query)?)?;

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
        Some("project:continuitydb:cli-query-envelope")
    );

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    Ok(())
}
```

- [ ] **Step 3: Add invalid envelope rejection test**

Add this test after the successful envelope checkout test:

```rust
#[test]
fn cli_checkout_query_rejects_invalid_query_envelope(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path("continuitydb-cli-checkout-query-invalid-envelope-store");
    let query_path = temp_store_path("continuitydb-cli-checkout-query-invalid-envelope-query");
    write_committed_store(&store_path, "project:continuitydb:cli-query-invalid-envelope")?;
    let envelope = QueryEnvelope {
        format: QUERY_ENVELOPE_FORMAT.to_string(),
        version: QUERY_ENVELOPE_FORMAT_VERSION + 1,
        query: ContinuityQuery::Checkout(CheckoutQuery::new(QueryTask::new(
            "stored-facts",
            "what is stored?",
        ))),
    };
    fs::write(&query_path, serde_json::to_vec(&envelope)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .failure()
        .stderr(contains("query envelope is invalid"));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    Ok(())
}
```

- [ ] **Step 4: Verify RED**

Run:

```bash
cargo test -p continuitydb-cli cli_checkout_query --all-features
```

Expected: FAIL because envelope files are not decoded by `checkout-query`.

## Task 2: CLI Decode Implementation

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`

- [ ] **Step 1: Import envelope decoder**

Change the query import in `crates/continuitydb-cli/src/main.rs` to:

```rust
use continuitydb_query::{decode_query_json, ContinuityQuery};
```

- [ ] **Step 2: Route checkout query file through decode helper**

Change `checkout_query_file` so it calls `decode_query_file`:

```rust
fn checkout_query_file(
    store_path: &PathBuf,
    query_path: &PathBuf,
) -> Result<continuitydb_checkout::CheckoutSlice, Box<dyn std::error::Error>> {
    let db = open_file_database(store_path)?;
    let encoded = std::fs::read(query_path)?;
    let query = decode_query_file(&encoded)?;
    db.checkout_continuity_query(query).map_err(Into::into)
}
```

- [ ] **Step 3: Add decode helper**

Add this helper near `checkout_query_file`:

```rust
fn decode_query_file(bytes: &[u8]) -> Result<ContinuityQuery, Box<dyn std::error::Error>> {
    if is_query_envelope_shape(bytes)? {
        return decode_query_json(bytes).map_err(Into::into);
    }
    serde_json::from_slice::<ContinuityQuery>(bytes).map_err(Into::into)
}
```

- [ ] **Step 4: Add envelope-shape helper**

Add this helper after `decode_query_file`:

```rust
fn is_query_envelope_shape(bytes: &[u8]) -> Result<bool, Box<dyn std::error::Error>> {
    let value = serde_json::from_slice::<serde_json::Value>(bytes)?;
    Ok(value.get("format").is_some() && value.get("version").is_some() && value.get("query").is_some())
}
```

- [ ] **Step 5: Verify GREEN**

Run:

```bash
cargo test -p continuitydb-cli cli_checkout_query --all-features
```

Expected: PASS.

- [ ] **Step 6: Commit implementation**

```bash
git add crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs docs/superpowers/plans/2026-05-20-cli-query-envelope.md
git commit -m "feat: support cli query envelopes"
```

## Task 3: Docs and Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-cli-query-envelope.md`

- [ ] **Step 1: Update README**

Add this current-scope bullet near the existing CLI query bullet:

```markdown
- CLI execution for versioned typed query envelopes.
```

- [ ] **Step 2: Update roadmap**

Add this CLI milestone after direct commit copy:

```markdown
14. Execute versioned typed query envelopes from the CLI. Extended `continuitydb checkout-query` to accept `continuitydb.query` envelope files while preserving raw typed query JSON compatibility.
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
git add README.md docs/roadmap.md docs/superpowers/plans/2026-05-20-cli-query-envelope.md
git commit -m "docs: record cli query envelopes"
```
