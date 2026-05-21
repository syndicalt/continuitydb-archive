# Local Model Contract Byte Metadata Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add durable schema and grammar byte-count metadata to local-model contract artifacts and validate it in bundle validation.

**Architecture:** Extend `LocalModelContractArtifacts` with schema and grammar byte counts computed from the contract strings written to disk. The existing benchmark report and bundle manifest projection already carry `contract_artifacts`, so adding fields to the JSON projection makes the metadata durable. Extend contract validation to compare declared byte counts to current file bytes before fingerprint checks.

**Tech Stack:** Rust workspace, `continuitydb-cli`, serde JSON, `assert_cmd`, cargo tests with the `local-model` feature.

---

### Task 1: Add durable contract byte metadata

**Files:**
- Modify: `crates/continuitydb-cli/tests/cli.rs`
- Modify: `crates/continuitydb-cli/src/main.rs`
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [x] **Step 1: Write failing output assertion**

Extend `cli_benchmark_local_model_contract_dir_writes_artifacts_and_supplies_grammar` to assert:

```rust
assert_eq!(
    json["contract_artifacts"]["schema_bytes"].as_u64(),
    Some(fs::read_to_string(&schema_path)?.len() as u64)
);
assert_eq!(
    json["contract_artifacts"]["grammar_bytes"].as_u64(),
    Some(fs::read_to_string(&grammar_path)?.len() as u64)
);
```

- [x] **Step 2: Run focused test to verify RED**

Run:

```sh
cargo test -p continuitydb-cli --features local-model cli_benchmark_local_model_contract_dir_writes_artifacts_and_supplies_grammar --test cli
```

Expected: fail because `schema_bytes` and `grammar_bytes` are not present.

- [x] **Step 3: Implement contract byte metadata output**

Modify `LocalModelContractArtifacts` to include:

```rust
schema_bytes: usize,
grammar_bytes: usize,
```

Set those fields in `write_local_model_contract_artifacts` from `schema.len()` and `grammar.len()`. Emit them from `local_model_contract_artifacts_json` as `schema_bytes` and `grammar_bytes`.

- [x] **Step 4: Run focused output test to verify GREEN**

Run:

```sh
cargo test -p continuitydb-cli --features local-model cli_benchmark_local_model_contract_dir_writes_artifacts_and_supplies_grammar --test cli
```

Expected: pass.

- [x] **Step 5: Write failing validation test**

Add `cli_validate_local_model_bundle_rejects_contract_artifact_byte_count_mismatch` near the existing contract artifact validation tests. The test must create a dry-run bundle, set `manifest["contract_artifacts"]["schema_bytes"] = 1`, write the manifest, run `validate-local-model-bundle`, and assert stderr contains `local model contract artifact schema byte count mismatch`.

- [x] **Step 6: Run focused validation test to verify RED**

Run:

```sh
cargo test -p continuitydb-cli --features local-model cli_validate_local_model_bundle_rejects_contract_artifact_byte_count_mismatch --test cli
```

Expected: fail because contract validation does not yet compare byte counts.

- [x] **Step 7: Implement byte-count validation**

In `validate_local_model_contract_artifacts`, after reading the schema and grammar files:

- compare `schema_bytes` to `schema_text.len()`
- compare `grammar_bytes` to `grammar_text.len()`
- return deterministic mismatch errors:
  - `local model contract artifact schema byte count mismatch`
  - `local model contract artifact grammar byte count mismatch`

- [x] **Step 8: Run focused validation test to verify GREEN**

Run:

```sh
cargo test -p continuitydb-cli --features local-model cli_validate_local_model_bundle_rejects_contract_artifact_byte_count_mismatch --test cli
```

Expected: pass.

- [x] **Step 9: Update docs**

Add `CLI local model contract artifact byte metadata` to `README.md` current scope and add roadmap item 110 in `docs/roadmap.md`.

- [x] **Step 10: Run verification gates**

Run:

```sh
cargo fmt --all
cargo fmt --all -- --check
git diff --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
```

- [ ] **Step 11: Commit**

Commit:

```sh
git add README.md crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs docs/roadmap.md docs/superpowers/plans/2026-05-20-local-model-contract-byte-metadata.md docs/superpowers/specs/2026-05-20-local-model-contract-byte-metadata-design.md
git commit -m "feat: add local-model contract byte metadata"
```
