# Local Model Validation Contract Failure Metadata Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add best-effort contract artifact evidence to local-model bundle validation failure reports.

**Architecture:** Extend `write_local_model_bundle_validation_failure_report` with a `contract_artifacts` JSON section produced by a helper that reads `benchmark-report.json`, locates archived schema and grammar artifact paths, and records current path, fingerprint, and byte-count evidence from disk. Cover the behavior through the existing CLI failure-report acceptance test style.

**Tech Stack:** Rust workspace, `continuitydb-cli`, serde JSON, `assert_cmd`, cargo tests with the `local-model` feature.

---

### Task 1: Report contract artifact metadata on validation failure

**Files:**
- Modify: `crates/continuitydb-cli/tests/cli.rs`
- Modify: `crates/continuitydb-cli/src/main.rs`
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [x] **Step 1: Write the failing acceptance test**

Add `cli_validate_local_model_bundle_failure_report_records_contract_artifact_metadata` near the local-model bundle validation failure-report tests. The test must:

- create a dry-run local-model benchmark bundle with `write_dry_run_local_model_bundle`
- read `contract_artifacts.schema_path` and `contract_artifacts.grammar_path` from `benchmark-report.json`
- overwrite the schema file with different content
- run `validate-local-model-bundle --artifact-dir <dir> --failure-report-path <path>`
- assert validation fails with `local model contract artifact schema fingerprint mismatch`
- read the failure report and assert `contract_artifacts.schema_path`, `schema_fingerprint`, `schema_bytes`, `grammar_path`, `grammar_fingerprint`, and `grammar_bytes` are present and describe current files

- [x] **Step 2: Run focused test to verify RED**

Run:

```sh
cargo test -p continuitydb-cli --features local-model cli_validate_local_model_bundle_failure_report_records_contract_artifact_metadata --test cli
```

Expected: fail because validation failure reports do not include `contract_artifacts`.

- [x] **Step 3: Implement contract artifact failure metadata**

Modify `write_local_model_bundle_validation_failure_report` to include:

```rust
let contract_artifacts = local_model_validation_failure_contract_artifacts_json(artifact_dir);
```

and emit:

```rust
"contract_artifacts": contract_artifacts,
```

Add `local_model_validation_failure_contract_artifacts_json(artifact_dir)` that:

- reads `artifact_dir/benchmark-report.json`
- parses JSON
- returns `Null` when `contract_artifacts` is null or unavailable
- reads `schema_path` and `grammar_path` from the report
- reads current schema and grammar file bytes
- returns schema and grammar paths, current fingerprints, and current byte counts

- [x] **Step 4: Run focused test to verify GREEN**

Run:

```sh
cargo test -p continuitydb-cli --features local-model cli_validate_local_model_bundle_failure_report_records_contract_artifact_metadata --test cli
```

Expected: pass.

- [x] **Step 5: Update docs**

Add `CLI local model bundle validation contract artifact failure metadata` to `README.md` current scope and add roadmap item 109 in `docs/roadmap.md`.

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
git add README.md crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs docs/roadmap.md docs/superpowers/plans/2026-05-20-local-model-validation-contract-failure-metadata.md docs/superpowers/specs/2026-05-20-local-model-validation-contract-failure-metadata-design.md
git commit -m "feat: report local-model contract failure evidence"
```
