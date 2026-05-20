# Local Model Bundle Validation Report Artifacts Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add successful validation report artifacts to `validate-local-model-bundle`.

**Architecture:** Extend the existing feature-gated CLI command with an optional report path. Build validation JSON through a helper so stdout and report-file output share one payload.

**Tech Stack:** Rust workspace, `continuitydb-cli`, `assert_cmd`, serde JSON, cargo tests with `local-model` feature.

---

### Task 1: Add report-path support to local-model bundle validation

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`
- Modify: `crates/continuitydb-cli/tests/cli.rs`
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [x] **Step 1: Write the failing acceptance test**

Add:

```rust
fn cli_validate_local_model_bundle_report_path_writes_validation_artifact()
```

The test should:

- create a dry-run local-model benchmark bundle
- run `validate-local-model-bundle --artifact-dir <dir> --report-path <report.json>`
- assert the command succeeds
- parse stdout and the report file
- assert the report file JSON equals stdout JSON
- assert `report_path` in the JSON equals the requested path

- [x] **Step 2: Run focused test to verify RED**

Run:

```sh
cargo test -p continuitydb-cli --features local-model cli_validate_local_model_bundle_report_path_writes_validation_artifact --test cli
```

Expected: fail because `validate-local-model-bundle` does not accept `--report-path`.

- [x] **Step 3: Implement local-model validation report output**

Add `report_path: Option<PathBuf>` to `ValidateLocalModelBundle`, include it in the output JSON, and call `write_pretty_json_file` when it is supplied.

- [x] **Step 4: Run focused test to verify GREEN**

Run:

```sh
cargo test -p continuitydb-cli --features local-model cli_validate_local_model_bundle_report_path_writes_validation_artifact --test cli
```

Expected: pass.

- [x] **Step 5: Update docs**

Add `CLI local model bundle validation report artifacts` to `README.md` current scope and add the next local-model roadmap milestone in `docs/roadmap.md`.

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
git add README.md crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs docs/roadmap.md docs/superpowers/plans/2026-05-20-local-model-bundle-validation-report-artifacts.md docs/superpowers/specs/2026-05-20-local-model-bundle-validation-report-artifacts-design.md
git commit -m "feat: write local-model validation reports"
```
