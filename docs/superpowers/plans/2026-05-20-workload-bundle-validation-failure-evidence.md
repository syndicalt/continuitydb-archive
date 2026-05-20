# Workload Bundle Validation Failure Evidence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Preserve best-effort workload fixture metadata in direct workload bundle validation failure reports.

**Architecture:** Extend the existing `write_workload_bundle_validation_failure_report` helper to include best-effort artifact metadata. Reuse the same path and FNV fingerprint shape already used in replay failure reports.

**Tech Stack:** Rust workspace, `continuitydb-cli`, `assert_cmd`, serde JSON, cargo tests.

---

### Task 1: Add fixture metadata to validation failure reports

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`
- Modify: `crates/continuitydb-cli/tests/cli.rs`
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [x] **Step 1: Write failing acceptance test**

Add:

```rust
fn cli_validate_workload_bundle_failure_report_records_fixture_metadata()
```

The test should:

- create a workload artifact bundle
- tamper with `workload-cells.json`
- run `validate-workload-bundle --artifact-dir <dir> --failure-report-path <failure.json>`
- parse the failure report
- assert `workload_artifacts.cells_path` equals the tampered fixture path
- assert `workload_artifacts.cells_fingerprint` starts with `fnv1a64:`
- assert `workload_artifacts.cells_bytes` is greater than zero
- assert request path, fingerprint, and bytes are present

- [x] **Step 2: Run focused test to verify RED**

Run:

```sh
cargo test -p continuitydb-cli cli_validate_workload_bundle_failure_report_records_fixture_metadata --test cli
```

Expected: fail because `workload_artifacts` is currently `null` in validation failure reports.

- [x] **Step 3: Implement best-effort failure metadata**

Add:

```rust
fn workload_validation_failure_artifacts_json(artifact_dir: &Path) -> serde_json::Value
```

The helper should read `workload-cells.json` and `checkout-request.json`. If both reads succeed, return path, fingerprint, and byte metadata. Otherwise return `serde_json::Value::Null`.

Use the helper in `write_workload_bundle_validation_failure_report`.

- [x] **Step 4: Run focused test to verify GREEN**

Run:

```sh
cargo test -p continuitydb-cli cli_validate_workload_bundle_failure_report_records_fixture_metadata --test cli
```

Expected: pass.

- [x] **Step 5: Update docs**

Add `CLI workload bundle validation failure evidence metadata` to `README.md` current scope and add the next workload milestone in `docs/roadmap.md`.

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
git add README.md crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs docs/roadmap.md docs/superpowers/plans/2026-05-20-workload-bundle-validation-failure-evidence.md docs/superpowers/specs/2026-05-20-workload-bundle-validation-failure-evidence-design.md
git commit -m "feat: report workload validation failure evidence"
```
