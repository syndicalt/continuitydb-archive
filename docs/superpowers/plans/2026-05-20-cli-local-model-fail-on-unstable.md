# CLI Local Model Fail-On-Unstable Gate Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an optional `benchmark-local-model --fail-on-unstable` gate for repeated-run local Steward benchmark instability.

**Architecture:** Reuse the existing CLI stability report path. Validate that the gate requires `--stability-trials`, include gate intent in dry-run JSON, and run stability before baseline recording so unstable candidates fail without mutating baseline files.

**Tech Stack:** Rust, Clap, existing `continuitydb-cli` local-model tests, existing `LocalModelStabilityReport` API.

---

### Task 1: Failing CLI Tests

**Files:**
- Modify: `crates/continuitydb-cli/tests/cli.rs`

- [x] Add a test proving `--fail-on-unstable` without `--stability-trials` fails and does not create a baseline file.
- [x] Add a dry-run test proving `stability_preflight.fail_on_unstable = true` when both flags are present.
- [x] Add a real-run test with a stateful local executable that changes rationale between repeated trials; expect command failure, an instability error, and no baseline file.
- [x] Run focused tests and verify RED:

```bash
cargo test -p continuitydb-cli cli_benchmark_local_model_fail_on_unstable --features local-model
```

### Task 2: CLI Implementation

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`

- [x] Add `fail_on_unstable: bool` to `LocalModelBenchmarkOptions`.
- [x] Add `--fail-on-unstable` to `Command::BenchmarkLocalModel`.
- [x] Pass the flag through command dispatch.
- [x] Reject `--fail-on-unstable` when `stability_trials` is absent.
- [x] Include `fail_on_unstable` in stability dry-run preflight JSON.
- [x] Return an error before baseline recording when the computed stability report is unstable and `fail_on_unstable` is set.
- [x] Run focused tests and verify GREEN:

```bash
cargo test -p continuitydb-cli cli_benchmark_local_model_fail_on_unstable --features local-model
```

### Task 3: Documentation and Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-cli-local-model-fail-on-unstable.md`

- [x] Add README current-scope bullet for CLI local-model instability failure gating.
- [x] Add roadmap Steward milestone for CLI local-model instability failure gating.
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
git commit -m "feat: add CLI local model instability gate"
```
