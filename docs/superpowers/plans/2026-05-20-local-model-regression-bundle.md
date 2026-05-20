# Local Model Regression Bundle Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Preserve complete artifact bundles for local-model baseline regression gate failures under `benchmark-local-model --artifact-dir`.

**Architecture:** Build the regression report before returning the existing non-zero regression error, then reuse the artifact bundle report helper. Keep baseline append after the regression gate so regressed runs remain unrecorded.

**Tech Stack:** Rust, clap, serde_json, existing `continuitydb-cli` local-model tests.

---

### Task 1: RED Test

**Files:**
- Modify: `crates/continuitydb-cli/tests/cli.rs`

- [x] Add a Unix local-model CLI test for `benchmark-local-model --artifact-dir --fail-on-regression`.
- [x] Record a passing compatible baseline with a runner that emits all fixed-suite proposals.
- [x] Run a second benchmark with a lower-quality runner that emits only one passing proposal.
- [x] Assert the second command exits non-zero and the baseline file still contains one record.
- [x] Assert `<artifact-dir>/benchmark-report.json` includes `baseline_comparison.regressed = true` and negative pass-count delta.
- [x] Assert `<artifact-dir>/local-model-benchmark.manifest.json` references report, prompts, responses, and nested response manifest.
- [x] Run focused test and verify RED:

```bash
cargo test -p continuitydb-cli cli_benchmark_local_model_artifact_dir_writes_regression_bundle --features local-model
```

### Task 2: Implementation

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`

- [x] In the regression-gate branch, build the same benchmark JSON that successful runs return.
- [x] Write artifact-dir report and root bundle manifest before returning the existing regression error.
- [x] Keep `store.append_baseline` after the regression gate.
- [x] Run focused test and verify GREEN:

```bash
cargo test -p continuitydb-cli cli_benchmark_local_model_artifact_dir_writes_regression_bundle --features local-model
```

### Task 3: Documentation and Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-local-model-regression-bundle.md`

- [x] Add README current-scope bullet for CLI local-model regression artifact bundles.
- [x] Add roadmap Steward milestone for regression artifact bundles.
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
git commit -m "feat: preserve local model regression bundles"
```
