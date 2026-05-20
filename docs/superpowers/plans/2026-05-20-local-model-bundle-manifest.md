# Local Model Bundle Manifest Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a top-level manifest to `benchmark-local-model --artifact-dir` bundles.

**Architecture:** Reuse the existing artifact JSON helpers to write a versioned root manifest after the report has been materialized. Thread the manifest metadata into stdout and report JSON through an optional artifact metadata field.

**Tech Stack:** Rust, clap, serde_json, existing `continuitydb-cli` local-model tests.

---

### Task 1: RED Tests

**Files:**
- Modify: `crates/continuitydb-cli/tests/cli.rs`

- [x] Extend the real-run artifact directory test to assert `<artifact-dir>/local-model-benchmark.manifest.json` exists.
- [x] Assert stdout and `benchmark-report.json` contain `bundle_manifest` with manifest path, fingerprint, and byte count.
- [x] Assert the manifest format is `continuitydb.local_model.benchmark_bundle` version 1.
- [x] Assert real-run manifest references the report, 9 prompts, 9 responses, and the nested response manifest.
- [x] Extend the dry-run artifact directory test to assert the manifest exists with 9 prompts, zero responses, and null nested response manifest.
- [x] Run focused tests and verify RED:

```bash
cargo test -p continuitydb-cli cli_benchmark_local_model_artifact_dir --features local-model
```

### Task 2: CLI Implementation

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`

- [x] Add a `LocalModelBundleManifest` metadata struct.
- [x] Add optional bundle manifest metadata to benchmark JSON.
- [x] Include `bundle_manifest` in dry-run and real-run benchmark JSON.
- [x] Write `<artifact-dir>/benchmark-report.json`, then `<artifact-dir>/local-model-benchmark.manifest.json`, then rewrite the report so it includes final manifest metadata.
- [x] Keep explicit `--report-path` writing the same JSON printed to stdout.
- [x] Run focused tests and verify GREEN:

```bash
cargo test -p continuitydb-cli cli_benchmark_local_model_artifact_dir --features local-model
```

### Task 3: Documentation and Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-local-model-bundle-manifest.md`

- [x] Add README current-scope bullet for CLI local-model benchmark bundle manifests.
- [x] Add roadmap Steward milestone for bundle manifests.
- [x] Run full verification:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

- [x] Commit with message:

```bash
git commit -m "feat: add local model bundle manifests"
```
