# Local Model Response Artifact Validation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `validate-local-model-bundle --artifact-dir` enforce raw local-model response artifact metadata listed in `responses/local-model-responses.manifest.json`.

**Architecture:** Extend the existing nested response manifest validator. After validating the nested manifest file path, byte count, and fingerprint, parse the nested manifest and validate each captured response artifact's path boundary, byte count, and fingerprint against the archived response file.

**Tech Stack:** Rust workspace, `continuitydb-cli`, feature-gated local-model CLI tests, Unix shell runner fixtures, serde JSON, assert_cmd.

---

### Task 1: Validate raw response artifact metadata

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`
- Modify: `crates/continuitydb-cli/tests/cli.rs`
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [x] **Step 1: Write failing CLI tests**

Add feature-gated Unix tests:

```rust
fn cli_validate_local_model_bundle_rejects_tampered_response_artifact()
fn cli_validate_local_model_bundle_rejects_response_artifact_byte_count_mismatch()
fn cli_validate_local_model_bundle_rejects_response_artifact_fingerprint_mismatch()
fn cli_validate_local_model_bundle_rejects_response_artifact_path_mismatch()
```

Each test should create a real `benchmark-local-model --artifact-dir` bundle using `passing_local_model_runner_script()`. Tests that mutate the nested response manifest should refresh the root bundle manifest's `response_artifact_manifest` byte count and fingerprint so validation reaches the per-response checks.

- [x] **Step 2: Run focused tests to verify RED**

Run:

```sh
cargo test -p continuitydb-cli --features local-model cli_validate_local_model_bundle_rejects_tampered_response_artifact
cargo test -p continuitydb-cli --features local-model cli_validate_local_model_bundle_rejects_response_artifact_byte_count_mismatch
cargo test -p continuitydb-cli --features local-model cli_validate_local_model_bundle_rejects_response_artifact_fingerprint_mismatch
cargo test -p continuitydb-cli --features local-model cli_validate_local_model_bundle_rejects_response_artifact_path_mismatch
```

Expected: failures because raw response artifacts are not yet validated.

- [x] **Step 3: Implement minimal production code**

Extend `validate_local_model_response_artifact_manifest` to parse the nested response manifest and call:

```rust
fn validate_local_model_response_artifact_files(
    artifact_dir: &Path,
    response_manifest: &serde_json::Value,
) -> Result<(), Box<dyn std::error::Error>>
```

For each artifact with `captured == true`, validate response path boundary, byte count, and fingerprint. For non-captured artifacts, skip file validation.

- [x] **Step 4: Run focused tests to verify GREEN**

Run the same focused tests and confirm they pass.

- [x] **Step 5: Update docs**

Record local-model response artifact metadata validation in `README.md` and `docs/roadmap.md`.

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
git add README.md crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs docs/roadmap.md docs/superpowers/plans/2026-05-20-local-model-response-artifact-validation.md docs/superpowers/specs/2026-05-20-local-model-response-artifact-validation-design.md
git commit -m "feat: validate local-model response artifacts"
```
