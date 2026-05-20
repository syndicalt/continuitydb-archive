# CLI Local Model Stability Reporting Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add operator-facing repeated-run stability reporting to `benchmark-local-model`.

**Architecture:** Extend the existing feature-gated benchmark command with `--stability-trials <N>`. Dry-runs report the intended stability configuration without executing a model; real runs include a serialized `LocalModelStabilityReport` alongside the existing benchmark JSON.

**Tech Stack:** Rust, Clap, serde_json, existing `continuitydb-cli` tests, existing `continuitydb-steward` local-model feature.

---

### Task 1: Failing CLI Tests

**Files:**
- Modify: `crates/continuitydb-cli/tests/cli.rs`

- [x] Add a dry-run test for `--stability-trials 3` that expects `stability_preflight.trials = 3`, `stability_preflight.will_execute = false`, and no baseline file.
- [x] Add a real benchmark test for `--stability-trials 2` that expects a top-level stable `stability` report and one recorded baseline.
- [x] Add a validation test for `--stability-trials 0` that expects command failure and no baseline file.
- [x] Run focused CLI tests and verify failure because the flag does not exist yet:

```bash
cargo test -p continuitydb-cli cli_benchmark_local_model_stability --features local-model
```

### Task 2: CLI Implementation

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`

- [x] Import `LocalModelStabilityReport`.
- [x] Add `stability_trials: Option<usize>` to `LocalModelBenchmarkOptions`.
- [x] Add `--stability-trials <N>` to `Command::BenchmarkLocalModel`.
- [x] Pass the option through command dispatch.
- [x] Reject zero trials before dry-run or execution with an operator-readable error.
- [x] Add dry-run `stability_preflight` JSON when configured.
- [x] Add real-run `stability` JSON when configured.
- [x] Run focused CLI tests and verify pass:

```bash
cargo test -p continuitydb-cli cli_benchmark_local_model_stability --features local-model
```

### Task 3: Documentation and Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-cli-local-model-stability-reporting.md`

- [x] Add README current-scope bullet for CLI local-model stability reporting.
- [x] Add roadmap Steward milestone for CLI local-model stability reporting.
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
git commit -m "feat: add CLI local model stability reporting"
```
