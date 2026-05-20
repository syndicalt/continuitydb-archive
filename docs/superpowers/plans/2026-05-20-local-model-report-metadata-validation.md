# Local Model Report Metadata Validation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a feature-gated CLI command that validates local-model benchmark bundle report metadata.

**Architecture:** Add `validate-local-model-bundle --artifact-dir` behind the existing `local-model` feature. Reuse the local-model manifest format and validate benchmark report byte/fingerprint metadata against a canonical report payload with `bundle_manifest` normalized to `null`, avoiding cyclic report/manifest fingerprints.

**Tech Stack:** Rust workspace, `continuitydb-cli`, clap, serde JSON, assert_cmd, cargo tests with `--features local-model`.

---

### Task 1: Validate local-model benchmark report metadata

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`
- Modify: `crates/continuitydb-cli/tests/cli.rs`
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [x] **Step 1: Write failing CLI tests**

Add three feature-gated tests:

```rust
fn cli_validate_local_model_bundle_accepts_report_metadata()
fn cli_validate_local_model_bundle_rejects_report_byte_count_mismatch()
fn cli_validate_local_model_bundle_rejects_report_fingerprint_mismatch()
```

Each test should create a dry-run `benchmark-local-model --artifact-dir` bundle. The success test should run `validate-local-model-bundle --artifact-dir` and assert the emitted report metadata. The failure tests should mutate only `benchmark_report_bytes` or `benchmark_report_fingerprint` in `local-model-benchmark.manifest.json`, run validation, and assert the expected error.

- [x] **Step 2: Run focused tests to verify RED**

Run:

```sh
cargo test -p continuitydb-cli --features local-model cli_validate_local_model_bundle_accepts_report_metadata
cargo test -p continuitydb-cli --features local-model cli_validate_local_model_bundle_rejects_report_byte_count_mismatch
cargo test -p continuitydb-cli --features local-model cli_validate_local_model_bundle_rejects_report_fingerprint_mismatch
```

Expected: failures because `validate-local-model-bundle` does not exist yet.

- [x] **Step 3: Implement minimal production code**

Add the feature-gated command variant and match arm. Implement:

```rust
fn local_model_benchmark_report_manifest_payload_text(report: &serde_json::Value) -> Result<String, serde_json::Error>
fn validate_local_model_bundle_manifest(artifact_dir: &Path) -> Result<LocalModelBundleManifest, Box<dyn std::error::Error>>
```

The validator should read the manifest and report, validate format/version/path/report byte count/report fingerprint, and return manifest metadata for the success JSON.

- [x] **Step 4: Run focused tests to verify GREEN**

Run the same three focused tests and confirm they pass.

- [x] **Step 5: Update docs**

Record local-model benchmark report metadata validation in `README.md` and `docs/roadmap.md`.

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
git add README.md crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs docs/roadmap.md docs/superpowers/plans/2026-05-20-local-model-report-metadata-validation.md docs/superpowers/specs/2026-05-20-local-model-report-metadata-validation-design.md
git commit -m "feat: validate local-model report metadata"
```
