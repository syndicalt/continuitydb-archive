# Local Model Artifact Directory Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `benchmark-local-model --artifact-dir` to create a coherent benchmark artifact bundle with one flag.

**Architecture:** Thread an optional artifact directory through the CLI options. Derive default contract, prompt, response, and report artifact paths from that directory while preserving explicit per-artifact flags as overrides.

**Tech Stack:** Rust, clap, serde_json, existing `continuitydb-cli` local-model tests.

---

### Task 1: RED Tests

**Files:**
- Modify: `crates/continuitydb-cli/tests/cli.rs`

- [x] Add a real-run CLI test for `benchmark-local-model --artifact-dir`.
- [x] Assert contracts, prompts, responses, response manifest, and `benchmark-report.json` are written under the artifact directory.
- [x] Assert stdout JSON matches `benchmark-report.json`.
- [x] Assert dry-run with `--artifact-dir` writes contracts, prompts, and report but no responses.
- [x] Run focused tests and verify RED:

```bash
cargo test -p continuitydb-cli cli_benchmark_local_model_artifact_dir --features local-model
```

### Task 2: CLI Implementation

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`

- [x] Add `--artifact-dir` to the `benchmark-local-model` clap command.
- [x] Add `artifact_dir` to `LocalModelBenchmarkOptions`.
- [x] Derive default `contracts`, `prompts`, and real-run `responses` directories from `artifact_dir`.
- [x] Write `<artifact-dir>/benchmark-report.json` after benchmark JSON is produced.
- [x] Preserve explicit `--contract-dir`, `--prompt-dir`, `--response-dir`, and `--report-path` behavior.
- [x] Run focused CLI tests and verify GREEN:

```bash
cargo test -p continuitydb-cli cli_benchmark_local_model_artifact_dir --features local-model
```

### Task 3: Documentation and Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-local-model-artifact-dir.md`

- [x] Add README current-scope bullet for CLI local-model artifact bundles.
- [x] Add roadmap Steward milestone for CLI local-model artifact bundles.
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
git commit -m "feat: add local model artifact bundles"
```
