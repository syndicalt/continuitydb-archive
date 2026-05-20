# CLI Query Files Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a CLI command that executes serialized typed Continuity query JSON files against file-backed stores.

**Architecture:** The CLI reads a `ContinuityQuery` JSON file, opens the existing file-backed database through `ContinuityDb::open_file`, executes `checkout_continuity_query`, and prints the existing `CheckoutSlice` JSON. The slice deliberately avoids query envelopes and text parsing.

**Tech Stack:** Rust 2021, `continuitydb-cli`, `continuitydb-query`, `continuitydb-api`, `serde_json`, `assert_cmd`.

---

## File Structure

- Modify `crates/continuitydb-cli/Cargo.toml`: add `continuitydb-query` as a production dependency.
- Modify `crates/continuitydb-cli/src/main.rs`: add the `checkout-query` command branch and helper.
- Modify `crates/continuitydb-cli/tests/cli.rs`: add end-to-end CLI tests using serialized `ContinuityQuery` values.
- Modify `README.md`: list CLI typed query-file execution in current scope.
- Modify `docs/roadmap.md`: add a Query Language or CLI milestone for query-file execution.
- Modify this plan: mark steps complete as executed.

## Task 1: Failing CLI Tests

**Files:**
- Modify: `crates/continuitydb-cli/Cargo.toml`
- Modify: `crates/continuitydb-cli/tests/cli.rs`

- [x] **Step 1: Add query crate test imports**

In `crates/continuitydb-cli/Cargo.toml`, add this under `[dev-dependencies]`:

```toml
continuitydb-query = { path = "../continuitydb-query" }
```

In `crates/continuitydb-cli/tests/cli.rs`, add this import near the other crate imports:

```rust
use continuitydb_query::{
    CheckoutQuery, ContinuityQuery, QueryOptimization, QueryRequirements, QueryReturnShape,
    QueryTask,
};
```

- [x] **Step 2: Add successful checkout-query test**

Add this test near the other CLI command tests:

```rust
#[test]
fn cli_checkout_query_executes_serialized_typed_query() -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path("continuitydb-cli-checkout-query-store");
    let query_path = temp_store_path("continuitydb-cli-checkout-query-query");
    write_committed_store(&store_path, "project:continuitydb:cli-query")?;
    let query = ContinuityQuery::Checkout(CheckoutQuery::new(QueryTask::new(
        "stored-facts",
        "what is stored?",
    ))
    .with_requirements(QueryRequirements {
        scope: Some(Scope::Project("continuitydb".to_string())),
        minimum_confidence: Confidence::new(0.7)?,
        token_budget: 1200,
        ..QueryRequirements::default()
    }));
    fs::write(&query_path, serde_json::to_vec(&query)?)?;

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
        Some("project:continuitydb:cli-query")
    );
    assert_eq!(json["audit_traces"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        json["audit_traces"][0]["evidence"].as_array().map(Vec::len),
        Some(1)
    );

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    Ok(())
}
```

- [x] **Step 3: Add unsupported semantics failure test**

Add this test after the successful checkout-query test:

```rust
#[test]
fn cli_checkout_query_rejects_unsupported_query_semantics(
) -> Result<(), Box<dyn std::error::Error>> {
    let store_path = temp_store_path("continuitydb-cli-checkout-query-unsupported-store");
    let query_path = temp_store_path("continuitydb-cli-checkout-query-unsupported-query");
    write_committed_store(&store_path, "project:continuitydb:cli-query-unsupported")?;
    let query = ContinuityQuery::Checkout(
        CheckoutQuery::new(QueryTask::new("stored-facts", "what is stored?"))
            .with_return_shape(QueryReturnShape::CellsOnly)
            .with_optimization(QueryOptimization::TokenCostOnly),
    );
    fs::write(&query_path, serde_json::to_vec(&query)?)?;

    Command::cargo_bin("continuitydb")?
        .arg("checkout-query")
        .arg(&store_path)
        .arg(&query_path)
        .assert()
        .failure()
        .stderr(contains("unsupported return shape"));

    fs::remove_file(store_path)?;
    fs::remove_file(query_path)?;
    Ok(())
}
```

- [x] **Step 4: Verify RED**

Run:

```bash
cargo test -p continuitydb-cli cli_checkout_query --all-features
```

Expected: FAIL because `checkout-query` is not a known command.

## Task 2: CLI Command Implementation

**Files:**
- Modify: `crates/continuitydb-cli/Cargo.toml`
- Modify: `crates/continuitydb-cli/src/main.rs`

- [x] **Step 1: Add production dependency**

In `crates/continuitydb-cli/Cargo.toml`, add this under `[dependencies]`:

```toml
continuitydb-query = { path = "../continuitydb-query" }
```

If `continuitydb-query` was added under `[dev-dependencies]` in Task 1, remove the dev-only entry because the CLI now uses the crate in production code.

- [x] **Step 2: Add query import**

In `crates/continuitydb-cli/src/main.rs`, add this import near the existing crate imports:

```rust
use continuitydb_query::ContinuityQuery;
```

- [x] **Step 3: Add command variant**

Add this variant to the `Command` enum after `DemoCheckout`:

```rust
    /// Execute a serialized typed Continuity query JSON file against a file-backed store.
    CheckoutQuery {
        /// Path to the JSONL file-backed store.
        store_path: PathBuf,
        /// Path to a serialized ContinuityQuery JSON file.
        query_path: PathBuf,
    },
```

- [x] **Step 4: Add run branch**

Add this `match` branch after `DemoCheckout`:

```rust
        Some(Command::CheckoutQuery {
            store_path,
            query_path,
        }) => {
            let slice = checkout_query_file(&store_path, &query_path)?;
            println!("{}", serde_json::to_string_pretty(&slice)?);
        }
```

- [x] **Step 5: Add helper function**

Add this helper near the file database helpers:

```rust
fn checkout_query_file(
    store_path: &PathBuf,
    query_path: &PathBuf,
) -> Result<continuitydb_checkout::CheckoutSlice, Box<dyn std::error::Error>> {
    let db = open_file_database(store_path)?;
    let encoded = std::fs::read(query_path)?;
    let query = serde_json::from_slice::<ContinuityQuery>(&encoded)?;
    db.checkout_continuity_query(query).map_err(Into::into)
}
```

- [x] **Step 6: Verify GREEN**

Run:

```bash
cargo test -p continuitydb-cli cli_checkout_query --all-features
```

Expected: PASS.

- [x] **Step 7: Commit implementation**

```bash
git add crates/continuitydb-cli/Cargo.toml crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs docs/superpowers/plans/2026-05-20-cli-query-files.md
git commit -m "feat: add cli query file checkout"
```

## Task 3: Docs and Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-cli-query-files.md`

- [x] **Step 1: Update README**

Add this bullet near the current query bullets:

```markdown
- CLI execution for serialized typed query files.
```

- [x] **Step 2: Update roadmap**

Add this Query Language milestone after portable typed query serialization:

```markdown
4. Execute serialized typed query files from the CLI. Implemented `continuitydb checkout-query <store-path> <query-path>` so saved `ContinuityQuery` JSON can materialize file-backed checkout slices through the native typed API before text query syntax exists.
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

Expected: all commands exit 0.

- [x] **Step 4: Commit docs**

```bash
git add README.md docs/roadmap.md docs/superpowers/plans/2026-05-20-cli-query-files.md
git commit -m "docs: record cli query files"
```
