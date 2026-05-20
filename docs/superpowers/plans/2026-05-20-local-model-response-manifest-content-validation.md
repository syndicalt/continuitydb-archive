# Local Model Response Manifest Content Validation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `validate-local-model-bundle --artifact-dir` reject nested response manifests whose artifact list no longer matches the archived benchmark report.

**Architecture:** Pass the parsed benchmark report into the response manifest validator. After existing nested manifest path, byte-count, fingerprint, and raw response file checks pass, compare `responses/local-model-responses.manifest.json["artifacts"]` against `benchmark-report.json["response_artifacts"]`.

**Tech Stack:** Rust workspace, `continuitydb-cli`, feature-gated local-model CLI tests, serde JSON, assert_cmd.

---

### Task 1: Validate response manifest content

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`
- Modify: `crates/continuitydb-cli/tests/cli.rs`
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [x] **Step 1: Write failing CLI tests**

Add feature-gated Unix tests:

```rust
fn cli_validate_local_model_bundle_rejects_response_manifest_case_name_mismatch()
fn cli_validate_local_model_bundle_rejects_response_manifest_capture_state_mismatch()
```

Each test should create a real bundle with `write_real_local_model_bundle`, mutate `responses/local-model-responses.manifest.json`, refresh the root manifest's `response_artifact_manifest` byte count and fingerprint, and assert:

```rust
.stderr(contains("local model response artifact manifest content mismatch"))
```

- [x] **Step 2: Run focused tests to verify RED**

Run:

```sh
cargo test -p continuitydb-cli --features local-model cli_validate_local_model_bundle_rejects_response_manifest_case_name_mismatch
cargo test -p continuitydb-cli --features local-model cli_validate_local_model_bundle_rejects_response_manifest_capture_state_mismatch
```

Expected: both tests fail because nested response manifest content is not yet compared with the benchmark report.

- [x] **Step 3: Implement minimal production code**

Update `validate_local_model_bundle_manifest` to call:

```rust
validate_local_model_response_artifact_manifest(artifact_dir, &manifest, &report)
```

Add:

```rust
fn validate_local_model_response_artifact_manifest_content(
    response_manifest: &serde_json::Value,
    benchmark_report: &serde_json::Value,
) -> Result<(), Box<dyn std::error::Error>>
```

The validator should compare format metadata and require exact equality between nested `artifacts` and root `response_artifacts`.

- [x] **Step 4: Run focused tests to verify GREEN**

Run:

```sh
cargo test -p continuitydb-cli --features local-model cli_validate_local_model_bundle_rejects_response_manifest_case_name_mismatch
cargo test -p continuitydb-cli --features local-model cli_validate_local_model_bundle_rejects_response_manifest_capture_state_mismatch
```

Expected: both tests pass.

- [x] **Step 5: Update docs**

Record response manifest content validation in `README.md` and `docs/roadmap.md`.

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
git add README.md crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs docs/roadmap.md docs/superpowers/plans/2026-05-20-local-model-response-manifest-content-validation.md docs/superpowers/specs/2026-05-20-local-model-response-manifest-content-validation-design.md
git commit -m "feat: validate local-model response manifest content"
```
