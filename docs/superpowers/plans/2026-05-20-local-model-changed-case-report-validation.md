# Local Model Changed-Case Report Validation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `validate-local-model-bundle --artifact-dir` enforce changed-case report metadata when a local-model benchmark bundle includes `changed-cases.json`.

**Architecture:** Reuse the existing local-model bundle validator. After root benchmark report validation, inspect the manifest's changed-case report fields; if present, read `changed-cases.json`, validate manifest path consistency, byte count, and fingerprint, then include changed-case report metadata in the success JSON.

**Tech Stack:** Rust workspace, `continuitydb-cli`, feature-gated local-model CLI tests, Unix shell runner fixtures, serde JSON, assert_cmd.

---

### Task 1: Validate changed-case report metadata

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`
- Modify: `crates/continuitydb-cli/tests/cli.rs`
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [x] **Step 1: Write failing CLI tests**

Add feature-gated Unix tests:

```rust
fn cli_validate_local_model_bundle_accepts_changed_case_report_metadata()
fn cli_validate_local_model_bundle_rejects_changed_case_report_path_mismatch()
fn cli_validate_local_model_bundle_rejects_changed_case_report_byte_count_mismatch()
fn cli_validate_local_model_bundle_rejects_changed_case_report_fingerprint_mismatch()
```

Each test should create a changed-case artifact bundle by recording a baseline with `passing_local_model_runner_script()`, replacing one rationale string, running `benchmark-local-model --compare-baseline --artifact-dir`, and then running `validate-local-model-bundle --artifact-dir`.

- [x] **Step 2: Run focused tests to verify RED**

Run:

```sh
cargo test -p continuitydb-cli --features local-model cli_validate_local_model_bundle_accepts_changed_case_report_metadata
cargo test -p continuitydb-cli --features local-model cli_validate_local_model_bundle_rejects_changed_case_report_path_mismatch
cargo test -p continuitydb-cli --features local-model cli_validate_local_model_bundle_rejects_changed_case_report_byte_count_mismatch
cargo test -p continuitydb-cli --features local-model cli_validate_local_model_bundle_rejects_changed_case_report_fingerprint_mismatch
```

Expected: failures because changed-case report metadata is not yet validated or emitted.

- [x] **Step 3: Implement minimal production code**

Extend `validate_local_model_bundle_manifest` to return both root benchmark report and changed-case report metadata:

```rust
fn validate_local_model_changed_case_report_manifest(
    artifact_dir: &Path,
    manifest: &serde_json::Value,
) -> Result<serde_json::Value, Box<dyn std::error::Error>>
```

The helper should return `serde_json::Value::Null` when no changed-case report is present. Otherwise, it should validate path, byte count, and fingerprint against `<artifact_dir>/changed-cases.json`.

- [x] **Step 4: Run focused tests to verify GREEN**

Run the same focused tests and confirm they pass.

- [x] **Step 5: Update docs**

Record local-model changed-case report metadata validation in `README.md` and `docs/roadmap.md`.

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
git add README.md crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs docs/roadmap.md docs/superpowers/plans/2026-05-20-local-model-changed-case-report-validation.md docs/superpowers/specs/2026-05-20-local-model-changed-case-report-validation-design.md
git commit -m "feat: validate local-model changed-case metadata"
```
