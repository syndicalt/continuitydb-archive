# Local Model Instability Bundle Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Preserve complete artifact bundles for local-model stability gate failures under `benchmark-local-model --artifact-dir`.

**Architecture:** When the stability gate fails and an artifact directory is configured, run one normal benchmark capture pass, materialize response artifacts, build benchmark JSON with the instability report attached, write the artifact bundle, and then return the existing stability error before baseline recording.

**Tech Stack:** Rust, clap, serde_json, existing `continuitydb-cli` local-model tests.

---

### Task 1: RED Test

**Files:**
- Modify: `crates/continuitydb-cli/tests/cli.rs`

- [x] Add a Unix local-model CLI test for `benchmark-local-model --artifact-dir --stability-trials 2 --fail-on-unstable`.
- [x] Use a counter-backed runner that changes output across repeated stability trials.
- [x] Assert the command exits non-zero and no baseline file is created.
- [x] Assert `<artifact-dir>/benchmark-report.json` includes `stability.stable = false`.
- [x] Assert `<artifact-dir>/local-model-benchmark.manifest.json` references report, responses, and nested response manifest.
- [x] Run focused test and verify RED:

```bash
cargo test -p continuitydb-cli cli_benchmark_local_model_artifact_dir_writes_instability_bundle --features local-model
```

### Task 2: Implementation

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`

- [x] In the instability-gate branch, when `artifact_dir` is present, run one normal benchmark capture pass.
- [x] Write response artifacts and nested response manifest through the existing artifact helpers.
- [x] Build benchmark JSON with the stability report attached.
- [x] Write artifact-dir report and root bundle manifest before returning the existing stability error.
- [x] Keep baseline recording after the instability gate.
- [x] Run focused test and verify GREEN:

```bash
cargo test -p continuitydb-cli cli_benchmark_local_model_artifact_dir_writes_instability_bundle --features local-model
```

### Task 3: Documentation and Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-local-model-instability-bundle.md`

- [x] Add README current-scope bullet for CLI local-model instability artifact bundles.
- [x] Add roadmap Steward milestone for instability artifact bundles.
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
git commit -m "feat: preserve local model instability bundles"
```
