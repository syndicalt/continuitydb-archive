# CLI Local Model Benchmark Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a feature-gated CLI command that records local Steward model benchmark baselines from a configured executable.

**Architecture:** Reuse `continuitydb-steward` benchmark primitives and expose a default fixed evaluation suite. Gate the CLI command behind a `local-model` feature so normal builds do not pull in the Steward local-model dependency. Keep command output structured JSON for CI and operator workflows.

**Tech Stack:** Rust 2021, clap, serde_json, `continuitydb-steward` with `local-model`.

---

### Task 1: RED Tests

**Files:**
- Modify: `crates/continuitydb-steward/src/lib.rs`
- Modify: `crates/continuitydb-cli/tests/cli.rs`

- [x] **Step 1: Add default suite test**

Add a `#[cfg(feature = "local-model")]` steward test named `default_steward_evaluation_suite_scores_valid_verification_proposal`. It should call `default_steward_evaluation_suite()`, run it against a static backend response with one `request_verification` proposal using `cell_id: null`, citation `continuitydb://evaluation/thin-evidence`, rationale containing `uncertainty`, and request `Gather additional source evidence.`, then assert the report passes and has one case.

- [x] **Step 2: Add CLI local-model benchmark test**

Add a `#[cfg(feature = "local-model")]` CLI smoke test named `cli_benchmark_local_model_records_baseline`. It should create a temporary executable script that prints valid Steward response JSON, run:

```bash
continuitydb benchmark-local-model \
  --candidate Qwen/Qwen2.5-0.5B-Instruct \
  --executable <script> \
  --model-path /models/qwen.gguf \
  --arg --temp \
  --arg 0 \
  --baseline-path <baseline>
```

Then assert output JSON reports `passed: true`, `passed_cases: 1`, candidate metadata, runtime executable, runtime arguments, and the baseline path. Read the baseline JSONL and assert it has one record with matching runtime metadata.

- [x] **Step 3: Run RED focused tests**

Run:

```bash
cargo test -p continuitydb-steward default_steward_evaluation_suite_scores_valid_verification_proposal --features local-model
cargo test -p continuitydb-cli cli_benchmark_local_model_records_baseline --features local-model
```

Expected: FAIL before implementation because `default_steward_evaluation_suite` and the CLI feature/command do not exist.

### Task 2: GREEN Implementation

**Files:**
- Modify: `crates/continuitydb-steward/src/local_model.rs`
- Modify: `crates/continuitydb-steward/src/lib.rs`
- Modify: `crates/continuitydb-cli/Cargo.toml`
- Modify: `crates/continuitydb-cli/src/main.rs`

- [x] **Step 1: Add default evaluation suite**

Add `pub fn default_steward_evaluation_suite() -> StewardEvaluationSuite` returning one fixed case:

- name: `insufficient evidence uncertainty`
- created_at: `2026-05-20T00:00:00Z`
- task: `Assess whether thin evidence needs verification.`
- evidence locator: `continuitydb://evaluation/thin-evidence`
- evidence text: `One weak source mentions the claim without corroboration.`
- expected action: `StewardAction::RequestVerification { cell_id: None, request: "Gather additional source evidence.".to_string() }`
- required citation: `continuitydb://evaluation/thin-evidence`
- required rationale term: `uncertainty`

Re-export it from `continuitydb-steward`.

- [x] **Step 2: Add CLI feature and dependency**

Add to `crates/continuitydb-cli/Cargo.toml`:

```toml
[features]
local-model = ["dep:continuitydb-steward", "continuitydb-steward/local-model"]

[dependencies]
continuitydb-steward = { path = "../continuitydb-steward", optional = true }
```

- [x] **Step 3: Add command and JSON output**

Add a `#[cfg(feature = "local-model")] BenchmarkLocalModel` command with:

- `--candidate`
- `--executable`
- `--model-path`
- repeated `--arg`
- `--baseline-path`
- `--compare-baseline`
- `--fail-on-regression`

Implement helper functions that resolve the fixed candidate by model ID, build `LocalExecutableRunnerConfig`, run `record_local_model_benchmark_baseline_with_regression`, optionally fail on regression, and print summary JSON.

- [x] **Step 4: Run GREEN focused tests**

Run the focused tests from Task 1. Expected: PASS.

### Task 3: Docs and Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-cli-local-model-benchmark.md`

- [x] **Step 1: Update README scope**

Add a scope bullet for the feature-gated CLI local model benchmark baseline recorder.

- [x] **Step 2: Update roadmap**

Add a Steward milestone noting that the CLI can record local-model benchmark baselines from configured executables.

- [x] **Step 3: Run full verification gate**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

Expected: PASS.

- [x] **Step 4: Mark plan complete**

Check off all completed boxes in this plan.

- [x] **Step 5: Commit**

Commit with:

```bash
git add README.md docs/roadmap.md docs/superpowers/specs/2026-05-20-cli-local-model-benchmark-design.md docs/superpowers/plans/2026-05-20-cli-local-model-benchmark.md crates/continuitydb-steward/src/local_model.rs crates/continuitydb-steward/src/lib.rs crates/continuitydb-cli/Cargo.toml crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs
git commit -m "feat: add local model benchmark cli"
```
