# Local Model Bundle Validation Failure Reports Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add structured failure report artifacts to `validate-local-model-bundle`.

**Architecture:** Extend the existing feature-gated local-model validation command with an optional failure-report path. Wrap validation so failures can write a deterministic JSON failure payload before returning the original error.

**Tech Stack:** Rust workspace, `continuitydb-cli`, `assert_cmd`, serde JSON, cargo tests with `local-model` feature.

---

### Task 1: Add local-model validation failure reports

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`
- Modify: `crates/continuitydb-cli/tests/cli.rs`
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [x] **Step 1: Write the failing acceptance test**

Add:

```rust
fn cli_validate_local_model_bundle_failure_report_path_records_validation_failure()
```

The test should:

- create a dry-run local-model benchmark bundle
- mutate `local-model-benchmark.manifest.json` so `benchmark_report_bytes` is wrong
- run `validate-local-model-bundle --artifact-dir <dir> --failure-report-path <failure.json>`
- assert stderr contains `local model benchmark manifest byte count mismatch`
- parse the failure report
- assert `failure.stage == "local_model_bundle_validation"`
- assert the failure message contains `local model benchmark manifest byte count mismatch`
- assert `artifact_dir` and `failure_report_path` match the requested paths

- [x] **Step 2: Run focused test to verify RED**

Run:

```sh
cargo test -p continuitydb-cli --features local-model cli_validate_local_model_bundle_failure_report_path_records_validation_failure --test cli
```

Expected: fail because `validate-local-model-bundle` does not accept `--failure-report-path`.

- [x] **Step 3: Implement local-model validation failure report output**

Add `failure_report_path: Option<PathBuf>` to `ValidateLocalModelBundle`. If validation fails and a failure report path is present, write a JSON payload with the failure stage and message before returning the validation error.

- [x] **Step 4: Run focused test to verify GREEN**

Run:

```sh
cargo test -p continuitydb-cli --features local-model cli_validate_local_model_bundle_failure_report_path_records_validation_failure --test cli
```

Expected: pass.

- [x] **Step 5: Update docs**

Add `CLI local model bundle validation failure report artifacts` to `README.md` current scope and add the next local-model roadmap milestone in `docs/roadmap.md`.

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

- [x] **Step 7: Commit**

Commit:

```sh
git add README.md crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs docs/roadmap.md docs/superpowers/plans/2026-05-20-local-model-bundle-validation-failure-reports.md docs/superpowers/specs/2026-05-20-local-model-bundle-validation-failure-reports-design.md
git commit -m "feat: write local-model validation failure reports"
```
