# Local Model Validation Response Artifact Failure Metadata Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Preserve best-effort raw response artifact metadata in local-model validation failure reports.

**Architecture:** Add a local-model validation failure helper that reads the response artifact manifest, walks declared captured response files under the bundle response directory, and emits current file evidence without performing validation. The failure report writer composes this evidence independently of manifest, benchmark report, changed-case report, and response-manifest metadata.

**Tech Stack:** Rust workspace, `continuitydb-cli`, `assert_cmd`, serde JSON, cargo tests with the `local-model` feature.

---

### Task 1: Add raw response artifact metadata to local-model validation failure reports

**Files:**
- Modify: `crates/continuitydb-cli/tests/cli.rs`
- Modify: `crates/continuitydb-cli/src/main.rs`
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [x] **Step 1: Write the failing acceptance test**

Add `cli_validate_local_model_bundle_failure_report_records_response_artifact_metadata` near the other local-model bundle validation failure report tests. The test must:

- create a real local-model benchmark bundle with `write_real_local_model_bundle`
- read the first captured response path from `responses/local-model-responses.manifest.json`
- overwrite that response file with different same-length content
- run `validate-local-model-bundle --artifact-dir <dir> --failure-report-path <failure.json>`
- assert stderr contains `local model response artifact fingerprint mismatch`
- assert `failure_report["response_artifacts"][0]["response_path"]` equals the tampered path
- assert `response_fingerprint` starts with `fnv1a64:`
- assert `response_bytes` equals the tampered response length

- [x] **Step 2: Run focused test to verify RED**

Run:

```sh
cargo test -p continuitydb-cli --features local-model cli_validate_local_model_bundle_failure_report_records_response_artifact_metadata --test cli
```

Expected: fail because `response_artifacts` is currently absent from local-model validation failure reports.

- [x] **Step 3: Implement best-effort response artifact metadata**

Add a helper in `crates/continuitydb-cli/src/main.rs` that:

- reads `responses/local-model-responses.manifest.json`
- parses the JSON
- reads the `artifacts` array
- for each artifact, copies `case_name`, `captured`, and `response_path`
- when `captured` is true and the path is inside `artifact_dir/responses`, reads the response file and records current `response_fingerprint` and `response_bytes`
- returns `serde_json::Value::Null` if the manifest or artifact list cannot be read

Call it from `write_local_model_bundle_validation_failure_report` and assign the result to `response_artifacts`.

- [x] **Step 4: Run focused test to verify GREEN**

Run:

```sh
cargo test -p continuitydb-cli --features local-model cli_validate_local_model_bundle_failure_report_records_response_artifact_metadata --test cli
```

Expected: pass.

- [x] **Step 5: Update docs**

Add `CLI local model bundle validation response artifact failure metadata` to `README.md` current scope and add the next local-model roadmap milestone in `docs/roadmap.md`.

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
git add README.md crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs docs/roadmap.md docs/superpowers/plans/2026-05-20-local-model-validation-response-artifact-failure-metadata.md docs/superpowers/specs/2026-05-20-local-model-validation-response-artifact-failure-metadata-design.md
git commit -m "feat: report local-model response artifact evidence"
```
