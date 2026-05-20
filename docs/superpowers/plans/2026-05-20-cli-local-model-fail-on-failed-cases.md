# CLI Local Model Failed-Case Gate Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `benchmark-local-model --fail-on-failed-cases` so fixed Steward evaluation failures fail before baseline recording.

**Architecture:** Replace the CLI's record-and-compare helper call with explicit in-memory baseline creation, optional compatible-baseline comparison, gate checks, and final append. Keep the public library helper unchanged.

**Tech Stack:** Rust, Clap, existing `continuitydb-cli` local-model tests, existing `continuitydb-steward` benchmark baseline and regression APIs.

---

### Task 1: RED Tests

**Files:**
- Modify: `crates/continuitydb-cli/tests/cli.rs`

- [x] Add a dry-run test proving `fail_on_failed_cases: true` appears in JSON and no baseline file is created.
- [x] Add a real-run test with incomplete proposals proving `--fail-on-failed-cases` exits non-zero and does not create a baseline file.
- [x] Add a real-run test with complete proposals proving `--fail-on-failed-cases` records one baseline when all cases pass.
- [x] Run focused tests and verify RED:

```bash
cargo test -p continuitydb-cli cli_benchmark_local_model_fail_on_failed_cases --features local-model
```

### Task 2: Implementation

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`

- [x] Add `fail_on_failed_cases: bool` to `LocalModelBenchmarkOptions`.
- [x] Add `--fail-on-failed-cases` to `Command::BenchmarkLocalModel`.
- [x] Pass the flag through command dispatch.
- [x] Include the flag in dry-run JSON.
- [x] Replace CLI helper recording with in-memory `LocalModelBenchmarkBaseline::from_report`.
- [x] Compare latest compatible baseline when requested.
- [x] Reject failed current evaluation before appending a baseline when the flag is set.
- [x] Append the current baseline only after failed-case and regression gates pass.
- [x] Run focused tests and verify GREEN:

```bash
cargo test -p continuitydb-cli cli_benchmark_local_model_fail_on_failed_cases --features local-model
```

### Task 3: Documentation and Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-cli-local-model-fail-on-failed-cases.md`

- [x] Add README current-scope bullet for CLI local-model failed-case failure gating.
- [x] Add roadmap Steward milestone for CLI local-model failed-case failure gating.
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
git commit -m "feat: add CLI local model failed case gate"
```
