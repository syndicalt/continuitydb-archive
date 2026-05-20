# Local Model Response Manifest Validation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `validate-local-model-bundle --artifact-dir` enforce nested response artifact manifest metadata when a local-model benchmark bundle includes raw response artifacts.

**Architecture:** Reuse the existing local-model bundle validator. After root benchmark report and changed-case validation, inspect `response_artifact_manifest`; if present, read `responses/local-model-responses.manifest.json`, validate path, byte count, and fingerprint, then include response manifest metadata in the success JSON.

**Tech Stack:** Rust workspace, `continuitydb-cli`, feature-gated local-model CLI tests, Unix shell runner fixtures, serde JSON, assert_cmd.

---

### Task 1: Validate nested response artifact manifest metadata

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`
- Modify: `crates/continuitydb-cli/tests/cli.rs`
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [x] **Step 1: Write failing CLI tests**

Add feature-gated Unix tests:

```rust
fn cli_validate_local_model_bundle_accepts_response_artifact_manifest_metadata()
fn cli_validate_local_model_bundle_rejects_response_artifact_manifest_path_mismatch()
fn cli_validate_local_model_bundle_rejects_response_artifact_manifest_byte_count_mismatch()
fn cli_validate_local_model_bundle_rejects_response_artifact_manifest_fingerprint_mismatch()
```

Each test should create a real `benchmark-local-model --artifact-dir` bundle using `passing_local_model_runner_script()`, mutate only the root bundle manifest when testing failures, and run `validate-local-model-bundle --artifact-dir`.

- [x] **Step 2: Run focused tests to verify RED**

Run:

```sh
cargo test -p continuitydb-cli --features local-model cli_validate_local_model_bundle_accepts_response_artifact_manifest_metadata
cargo test -p continuitydb-cli --features local-model cli_validate_local_model_bundle_rejects_response_artifact_manifest_path_mismatch
cargo test -p continuitydb-cli --features local-model cli_validate_local_model_bundle_rejects_response_artifact_manifest_byte_count_mismatch
cargo test -p continuitydb-cli --features local-model cli_validate_local_model_bundle_rejects_response_artifact_manifest_fingerprint_mismatch
```

Expected: failures because response artifact manifest metadata is not yet validated or emitted.

- [x] **Step 3: Implement minimal production code**

Extend `LocalModelBundleValidation` and `validate_local_model_bundle_manifest` to include response artifact manifest metadata:

```rust
fn validate_local_model_response_artifact_manifest(
    artifact_dir: &Path,
    manifest: &serde_json::Value,
) -> Result<serde_json::Value, Box<dyn std::error::Error>>
```

The helper should return `serde_json::Value::Null` when no response artifact manifest is present. Otherwise, it should validate path, byte count, and fingerprint against `<artifact_dir>/responses/local-model-responses.manifest.json`.

- [x] **Step 4: Run focused tests to verify GREEN**

Run the same focused tests and confirm they pass.

- [x] **Step 5: Update docs**

Record local-model response artifact manifest metadata validation in `README.md` and `docs/roadmap.md`.

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
git add README.md crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs docs/roadmap.md docs/superpowers/plans/2026-05-20-local-model-response-manifest-validation.md docs/superpowers/specs/2026-05-20-local-model-response-manifest-validation-design.md
git commit -m "feat: validate local-model response manifest"
```
