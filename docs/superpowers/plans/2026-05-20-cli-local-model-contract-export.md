# CLI Local Model Contract Export Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a feature-gated CLI command that writes local Steward model JSON Schema and GBNF grammar files.

**Architecture:** Reuse the existing `continuitydb-steward` local-model contract accessors from the CLI's `local-model` feature. Keep the command read-only and artifact-oriented: it writes operator-selected files and emits structured JSON with paths and schema version.

**Tech Stack:** Rust 2021, clap, serde_json, `continuitydb-steward` with `local-model`.

---

### Task 1: RED Tests

**Files:**
- Modify: `crates/continuitydb-cli/tests/cli.rs`

- [x] **Step 1: Add contract export CLI test**

Add a `#[cfg(feature = "local-model")]` test named `cli_local_model_contract_writes_schema_and_grammar`. It should run:

```bash
continuitydb local-model-contract \
  --schema-path <schema-path> \
  --grammar-path <grammar-path>
```

Then assert:

- output JSON `schema_version` is `1`
- output JSON `schema_path` and `grammar_path` match the temp paths
- schema file parses as JSON
- schema `$id` is `https://continuitydb.dev/schemas/local-model-response.schema.json`
- schema `x-continuitydb-schema-version` is `1`
- grammar file contains `root ::= response`
- grammar file contains `request-verification-action`

- [x] **Step 2: Run RED focused test**

Run:

```bash
cargo test -p continuitydb-cli cli_local_model_contract_writes_schema_and_grammar --features local-model
```

Expected: FAIL before implementation because the `local-model-contract` command does not exist.

### Task 2: GREEN Implementation

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`

- [x] **Step 1: Import contract accessors**

Extend the feature-gated `continuitydb_steward` imports with:

- `local_model_response_gbnf_grammar`
- `local_model_response_json_schema`
- `LOCAL_MODEL_RESPONSE_SCHEMA_VERSION`

- [x] **Step 2: Add CLI command**

Add a `#[cfg(feature = "local-model")] LocalModelContract` command with:

- `--schema-path <PathBuf>`
- `--grammar-path <PathBuf>`

- [x] **Step 3: Implement artifact writing**

Add helper `write_local_model_contract_json(schema_path: &Path, grammar_path: &Path) -> Result<serde_json::Value, Box<dyn std::error::Error>>` that writes the schema and grammar strings, then returns:

```json
{
  "schema_version": 1,
  "schema_path": "...",
  "grammar_path": "..."
}
```

- [x] **Step 4: Run GREEN focused test**

Run the focused test from Task 1. Expected: PASS.

### Task 3: Docs and Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-cli-local-model-contract-export.md`

- [x] **Step 1: Update README scope**

Add a current-scope bullet for feature-gated CLI local model contract export.

- [x] **Step 2: Update roadmap**

Add a Steward milestone noting the CLI can write local-model response schema and grammar artifacts.

- [x] **Step 3: Run full verification gate**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

Expected: PASS.

- [x] **Step 4: Mark plan complete**

Check off all completed boxes in this plan.

- [x] **Step 5: Commit**

Commit with:

```bash
git add README.md docs/roadmap.md docs/superpowers/specs/2026-05-20-cli-local-model-contract-export-design.md docs/superpowers/plans/2026-05-20-cli-local-model-contract-export.md crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs
git commit -m "feat: export local model contracts from cli"
```
