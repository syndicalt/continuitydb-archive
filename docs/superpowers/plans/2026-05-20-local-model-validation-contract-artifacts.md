# Local Model Validation Contract Artifacts Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Validate archived contract artifacts in local-model benchmark bundles.

**Architecture:** Add a contract artifact validation helper called from `validate_local_model_bundle_manifest`. It compares manifest/report contract metadata, confines schema and grammar paths to the bundle contract directory, validates fingerprints against current file bytes, and returns the validated contract metadata for success output.

**Tech Stack:** Rust workspace, `continuitydb-cli`, `assert_cmd`, serde JSON, cargo tests with the `local-model` feature.

---

### Task 1: Validate contract artifacts in local-model bundle validation

**Files:**
- Modify: `crates/continuitydb-cli/tests/cli.rs`
- Modify: `crates/continuitydb-cli/src/main.rs`
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [x] **Step 1: Write the failing acceptance test**

Add `cli_validate_local_model_bundle_rejects_tampered_contract_artifact` near the local-model bundle validation tests. The test must:

- create a dry-run local-model benchmark bundle with `write_dry_run_local_model_bundle`
- read `contract_artifacts.schema_path` from `benchmark-report.json`
- overwrite that schema file with different content
- run `validate-local-model-bundle --artifact-dir <dir>`
- assert validation fails with `local model contract artifact schema fingerprint mismatch`

- [x] **Step 2: Run focused test to verify RED**

Run:

```sh
cargo test -p continuitydb-cli --features local-model cli_validate_local_model_bundle_rejects_tampered_contract_artifact --test cli
```

Expected: fail because validation currently accepts tampered contract artifact files.

- [x] **Step 3: Implement contract artifact validation**

Modify `LocalModelBundleValidation` to include:

```rust
contract_artifacts: serde_json::Value,
```

Add `validate_local_model_contract_artifacts(artifact_dir, manifest, benchmark_report)` that:

- returns `Null` if `manifest["contract_artifacts"]` is null
- rejects when `manifest["contract_artifacts"] != benchmark_report["contract_artifacts"]`
- requires `schema_path` and `grammar_path` under `artifact_dir/contracts`
- reads both files
- compares `schema_fingerprint` and `grammar_fingerprint`
- returns the benchmark report `contract_artifacts` value

Call this helper from `validate_local_model_bundle_manifest` and emit it from successful validation JSON as `contract_artifacts`.

- [x] **Step 4: Run focused test to verify GREEN**

Run:

```sh
cargo test -p continuitydb-cli --features local-model cli_validate_local_model_bundle_rejects_tampered_contract_artifact --test cli
```

Expected: pass.

- [x] **Step 5: Update docs**

Add `CLI local model contract artifact metadata validation` to `README.md` current scope and add the next local-model roadmap milestone in `docs/roadmap.md`.

- [x] **Step 6: Run verification gates**

Run:

```sh
cargo fmt --all
cargo fmt --all -- --check
git diff --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
```

- [ ] **Step 7: Commit**

Commit:

```sh
git add README.md crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs docs/roadmap.md docs/superpowers/plans/2026-05-20-local-model-validation-contract-artifacts.md docs/superpowers/specs/2026-05-20-local-model-validation-contract-artifacts-design.md
git commit -m "feat: validate local-model contract artifacts"
```
