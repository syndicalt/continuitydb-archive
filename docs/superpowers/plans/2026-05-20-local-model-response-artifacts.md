# Local Model Response Artifacts Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `benchmark-local-model --response-dir` so real local model benchmark runs persist raw per-case model stdout.

**Architecture:** Add response capture to the Steward evaluation path without changing proposal validation. The CLI writes captured responses to files and reports artifact metadata alongside existing benchmark JSON.

**Tech Stack:** Rust, clap, serde_json, existing `continuitydb-steward` and `continuitydb-cli` local-model tests.

---

### Task 1: RED Tests

**Files:**
- Modify: `crates/continuitydb-cli/tests/cli.rs`

- [x] Add a passing real-run CLI test using `--response-dir`.
- [x] Assert the benchmark JSON includes nine `response_artifacts`.
- [x] Assert the first artifact has case name `insufficient evidence uncertainty`, a fingerprint, nonzero byte count, and a readable file containing the raw JSON proposal.
- [x] Run focused test and verify RED:

```bash
cargo test -p continuitydb-cli cli_benchmark_local_model_response_dir --features local-model
```

### Task 2: Steward Capture API

**Files:**
- Modify: `crates/continuitydb-steward/src/local_model.rs`
- Modify: `crates/continuitydb-steward/src/lib.rs`

- [x] Add `LocalModelSteward::raw_response` so evaluation can capture backend stdout before decoding.
- [x] Add `StewardEvaluationCaseResponse` with case name and optional raw response text.
- [x] Add `StewardEvaluationSuite::evaluate_with_responses`.
- [x] Add `LocalModelBenchmark::run_with_responses`.
- [x] Re-export `StewardEvaluationCaseResponse`.

### Task 3: CLI Implementation

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`

- [x] Add `--response-dir` to `benchmark-local-model`.
- [x] Use `run_with_responses` when response artifacts are requested.
- [x] Write one response file per captured case.
- [x] Include `response_artifacts` in dry-run, success, and failure-report JSON.
- [x] Run focused tests and verify GREEN:

```bash
cargo test -p continuitydb-cli cli_benchmark_local_model_response_dir --features local-model
```

### Task 4: Documentation and Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-local-model-response-artifacts.md`

- [x] Add README current-scope bullet for CLI local-model raw response artifacts.
- [x] Add roadmap Steward milestone for CLI local-model raw response artifacts.
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
git commit -m "feat: add CLI local model response artifacts"
```
