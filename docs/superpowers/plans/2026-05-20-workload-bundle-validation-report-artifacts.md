# Workload Bundle Validation Report Artifacts Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Persist success and failure JSON reports for direct workload bundle validation.

**Architecture:** Extend the existing `ValidateWorkloadBundle` CLI command with optional report paths. Factor validation output construction into a helper, write the success report after validation, and write a structured failure report before returning validation errors when `--failure-report-path` is supplied.

**Tech Stack:** Rust workspace, `continuitydb-cli`, `assert_cmd`, serde JSON, cargo tests.

---

### Task 1: Add workload bundle validation report paths

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`
- Modify: `crates/continuitydb-cli/tests/cli.rs`
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [x] **Step 1: Write failing acceptance tests**

Add:

```rust
fn cli_validate_workload_bundle_report_path_writes_validation_artifact()
```

The test should:

- create a workload artifact bundle
- run `validate-workload-bundle --artifact-dir <dir> --report-path <report.json>`
- parse stdout and the report file
- assert they are equal
- assert the report contains `manifest.manifest_path`

Add:

```rust
fn cli_validate_workload_bundle_failure_report_path_records_validation_failure()
```

The test should:

- create a workload artifact bundle
- tamper with `workload-cells.json`
- run `validate-workload-bundle --artifact-dir <dir> --failure-report-path <failure.json>`
- assert stderr contains `workload artifact manifest fingerprint mismatch`
- parse the failure report
- assert `failure.stage == "workload_bundle_validation"`
- assert `failure.message == "workload artifact manifest fingerprint mismatch"`

- [x] **Step 2: Run focused tests to verify RED**

Run:

```sh
cargo test -p continuitydb-cli cli_validate_workload_bundle_ --test cli
```

Expected: new tests fail because `validate-workload-bundle` does not accept `--report-path` or `--failure-report-path`.

- [x] **Step 3: Add CLI options and report writing**

Extend `ValidateWorkloadBundle`:

```rust
ValidateWorkloadBundle {
    #[arg(long = "artifact-dir")]
    artifact_dir: PathBuf,
    #[arg(long = "report-path")]
    report_path: Option<PathBuf>,
    #[arg(long = "failure-report-path")]
    failure_report_path: Option<PathBuf>,
}
```

Add a helper that returns validation output JSON. On success, write `--report-path` if supplied. On failure, write the failure report if supplied, then return the original error.

- [x] **Step 4: Run focused tests to verify GREEN**

Run:

```sh
cargo test -p continuitydb-cli cli_validate_workload_bundle_ --test cli
```

Expected: pass.

- [x] **Step 5: Update docs**

Add `CLI workload bundle validation report artifacts` to `README.md` current scope and add the next workload milestone in `docs/roadmap.md`.

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
git add README.md crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs docs/roadmap.md docs/superpowers/plans/2026-05-20-workload-bundle-validation-report-artifacts.md docs/superpowers/specs/2026-05-20-workload-bundle-validation-report-artifacts-design.md
git commit -m "feat: write workload validation reports"
```
