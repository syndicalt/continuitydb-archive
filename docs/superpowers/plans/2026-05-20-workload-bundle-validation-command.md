# Workload Bundle Validation Command Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a standalone CLI command that validates archived workload artifact bundles without replaying them.

**Architecture:** Reuse the existing workload manifest validation function used by `replay-workload --require-manifest`. Add a lightweight validation result struct and CLI command branch that reads the bundle, validates report/fixture metadata, and prints structured JSON.

**Tech Stack:** Rust workspace, `continuitydb-cli`, `assert_cmd`, serde JSON, cargo tests.

---

### Task 1: Add `validate-workload-bundle`

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`
- Modify: `crates/continuitydb-cli/tests/cli.rs`
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [x] **Step 1: Write failing acceptance tests**

Add:

```rust
fn cli_validate_workload_bundle_accepts_manifest_metadata()
```

The test should:

- create a temporary artifact directory
- run `continuitydb measure-workload --kernel memory --cells 8 --token-budget 400 --artifact-dir <dir>`
- run `continuitydb validate-workload-bundle --artifact-dir <dir>`
- assert the output includes:
  - `artifact_dir`
  - `manifest.manifest_path`
  - `workload_report.report_path`
  - `workload_artifacts.cells_path`
  - `workload_artifacts.checkout_request_path`

Add:

```rust
fn cli_validate_workload_bundle_rejects_tampered_fixture()
```

The test should:

- create a workload artifact bundle
- mutate `workload-cells.json`
- run `continuitydb validate-workload-bundle --artifact-dir <dir>`
- assert failure stderr contains `workload artifact manifest fingerprint mismatch`

- [x] **Step 2: Run focused tests to verify RED**

Run:

```sh
cargo test -p continuitydb-cli cli_validate_workload_bundle_ --test cli
```

Expected: fail because the `validate-workload-bundle` subcommand does not exist.

- [x] **Step 3: Add the CLI command and validation output**

Add a `ValidateWorkloadBundle` command variant:

```rust
ValidateWorkloadBundle {
    #[arg(long = "artifact-dir")]
    artifact_dir: PathBuf,
}
```

Add:

```rust
struct WorkloadBundleValidation {
    manifest: WorkloadBundleManifest,
    workload_report: serde_json::Value,
    workload_artifacts: serde_json::Value,
}
```

Add a helper that reads `workload-cells.json` and `checkout-request.json`, calls `validate_workload_artifact_manifest`, and returns the validation metadata.

- [x] **Step 4: Run focused tests to verify GREEN**

Run:

```sh
cargo test -p continuitydb-cli cli_validate_workload_bundle_ --test cli
```

Expected: pass.

- [x] **Step 5: Update docs**

Add `CLI workload bundle validation command` to `README.md` current scope and add the next workload milestone in `docs/roadmap.md`.

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
git add README.md crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs docs/roadmap.md docs/superpowers/plans/2026-05-20-workload-bundle-validation-command.md docs/superpowers/specs/2026-05-20-workload-bundle-validation-command-design.md
git commit -m "feat: validate workload bundles"
```
