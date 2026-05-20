# Local Model Failure Bundle Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Preserve complete artifact bundles for fixed evaluation failures under `benchmark-local-model --artifact-dir`.

**Architecture:** Reuse the existing report and bundle-manifest writers inside the fixed-evaluation failure branch. The branch writes the artifact-directory report first, writes the root bundle manifest, rewrites the report with manifest metadata, mirrors that final JSON to `--failure-report-path` when requested, and then returns the existing non-zero error.

**Tech Stack:** Rust, clap, serde_json, existing `continuitydb-cli` local-model tests.

---

### Task 1: RED Test

**Files:**
- Modify: `crates/continuitydb-cli/tests/cli.rs`

- [x] Add a Unix local-model CLI test for `benchmark-local-model --artifact-dir --fail-on-failed-cases`.
- [x] Use a runner that emits only one passing proposal so the fixed suite fails.
- [x] Assert the command exits non-zero and no baseline file is created.
- [x] Assert `<artifact-dir>/benchmark-report.json` exists and reports failed cases.
- [x] Assert `<artifact-dir>/local-model-benchmark.manifest.json` exists and references report, prompts, responses, and nested response manifest.
- [x] Run focused test and verify RED:

```bash
cargo test -p continuitydb-cli cli_benchmark_local_model_artifact_dir_writes_failed_case_bundle --features local-model
```

### Task 2: Implementation

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`

- [x] Add helper to write an artifact-directory benchmark report plus root bundle manifest and return final report JSON.
- [x] Use the helper in the command wrapper for successful/dry-run artifact-dir output.
- [x] Use the helper in the fixed-evaluation failure branch before returning the existing error.
- [x] Mirror final failure JSON to explicit `--failure-report-path` when provided.
- [x] Run focused test and verify GREEN:

```bash
cargo test -p continuitydb-cli cli_benchmark_local_model_artifact_dir_writes_failed_case_bundle --features local-model
```

### Task 3: Documentation and Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-local-model-failure-bundle.md`

- [x] Add README current-scope bullet for CLI local-model fixed-failure artifact bundles.
- [x] Add roadmap Steward milestone for fixed-failure artifact bundles.
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
git commit -m "feat: preserve local model failure bundles"
```
