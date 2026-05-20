# CLI Local Model Failure Report Artifact Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `benchmark-local-model --failure-report-path` so failed fixed-suite gates can write structured JSON without recording a baseline.

**Architecture:** Extend the existing CLI benchmark options with an optional report path. On failed-case gate rejection, serialize the same benchmark JSON report shape to that path before returning the gate error.

**Tech Stack:** Rust, Clap, serde_json, existing `continuitydb-cli` local-model tests.

---

### Task 1: RED Tests

**Files:**
- Modify: `crates/continuitydb-cli/tests/cli.rs`

- [x] Add a dry-run test proving `failure_report_path` appears in JSON and no file is created.
- [x] Add a failing fixed evaluation test proving the report file is written, contains failed case details, and no baseline is created.
- [x] Add a passing fixed evaluation test proving no failure report is written and one baseline is recorded.
- [x] Run focused tests and verify RED:

```bash
cargo test -p continuitydb-cli cli_benchmark_local_model_failure_report --features local-model
```

### Task 2: Implementation

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`

- [x] Add `failure_report_path: Option<&Path>` to `LocalModelBenchmarkOptions`.
- [x] Add `--failure-report-path <PATH>` to `Command::BenchmarkLocalModel`.
- [x] Pass the path through command dispatch.
- [x] Include the path in dry-run JSON.
- [x] When `--fail-on-failed-cases` rejects the current in-memory baseline, write `local_model_benchmark_json` to the report path if configured.
- [x] Ensure successful runs do not write a failure report.
- [x] Run focused tests and verify GREEN:

```bash
cargo test -p continuitydb-cli cli_benchmark_local_model_failure_report --features local-model
```

### Task 3: Documentation and Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-cli-local-model-failure-report-artifact.md`

- [x] Add README current-scope bullet for CLI local-model failure report artifacts.
- [x] Add roadmap Steward milestone for CLI local-model failure report artifacts.
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
git commit -m "feat: add CLI local model failure report artifact"
```
