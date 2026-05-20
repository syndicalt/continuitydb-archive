# Local Model Validation Changed-Case Failure Metadata Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Preserve best-effort changed-case report metadata in local-model validation failure reports.

**Architecture:** Add a local-model failure metadata helper for `changed-cases.json`. The failure report writer composes manifest, benchmark report, and changed-case metadata independently so missing optional artifacts remain null.

**Tech Stack:** Rust workspace, `continuitydb-cli`, `assert_cmd`, serde JSON, cargo tests with `local-model` feature.

---

### Task 1: Add changed-case metadata to local-model validation failure reports

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`
- Modify: `crates/continuitydb-cli/tests/cli.rs`
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [x] **Step 1: Write the failing acceptance test**

Add:

```rust
fn cli_validate_local_model_bundle_failure_report_records_changed_case_metadata()
```

The test should:

- create a changed-case local-model benchmark bundle with `write_changed_case_local_model_bundle`
- mutate `local-model-benchmark.manifest.json` so `benchmark_report_bytes` is wrong
- run `validate-local-model-bundle --artifact-dir <dir> --failure-report-path <failure.json>`
- assert stderr contains `local model benchmark manifest byte count mismatch`
- parse the failure report
- assert `changed_case_report.report_path` equals `changed-cases.json`
- assert `changed_case_report.report_fingerprint` starts with `fnv1a64:`
- assert `changed_case_report.report_bytes` is greater than zero

- [x] **Step 2: Run focused test to verify RED**

Run:

```sh
cargo test -p continuitydb-cli --features local-model cli_validate_local_model_bundle_failure_report_records_changed_case_metadata --test cli
```

Expected: fail because `changed_case_report` is currently `null` in local-model validation failure reports.

- [x] **Step 3: Implement best-effort changed-case metadata**

Add:

```rust
fn local_model_validation_failure_changed_case_report_json(artifact_dir: &Path) -> serde_json::Value
```

Use it from `write_local_model_bundle_validation_failure_report`.

- [x] **Step 4: Run focused test to verify GREEN**

Run:

```sh
cargo test -p continuitydb-cli --features local-model cli_validate_local_model_bundle_failure_report_records_changed_case_metadata --test cli
```

Expected: pass.

- [x] **Step 5: Update docs**

Add `CLI local model bundle validation changed-case failure metadata` to `README.md` current scope and add the next local-model roadmap milestone in `docs/roadmap.md`.

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
git add README.md crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs docs/roadmap.md docs/superpowers/plans/2026-05-20-local-model-validation-changed-case-failure-metadata.md docs/superpowers/specs/2026-05-20-local-model-validation-changed-case-failure-metadata-design.md
git commit -m "feat: report local-model changed-case evidence"
```
