# Workload Baseline CLI Gate Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a CLI comparison and optional failure gate for workload measurement baselines.

**Architecture:** Reuse `continuitydb-workload` baseline comparison APIs from the CLI. `measure-workload` loads the latest matching baseline before appending a new record, includes comparison metadata in JSON output, and optionally returns an error when regressions are present.

**Tech Stack:** Rust 2021, clap, assert_cmd CLI tests, serde_json output.

---

### Task 1: RED CLI Tests

**Files:**
- Modify: `crates/continuitydb-cli/tests/cli.rs`

- [x] **Step 1: Add failing CLI tests**

Add tests for successful comparison output, failure on deterministic regression, and missing baseline path validation.

- [x] **Step 2: Verify RED**

Run: `cargo test -p continuitydb-cli cli_measure_workload_compares_baseline`

Expected: FAIL because the new flags do not exist.

### Task 2: GREEN CLI Implementation

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`

- [x] **Step 1: Add CLI options and comparison plumbing**

Add `compare_baseline`, `max_elapsed_growth_percent`, and `fail_on_regression` fields to the command and internal options.

- [x] **Step 2: Load latest matching baseline before recording**

Use `FileWorkloadBaselineStore::latest_matching` with the current label and kernel name before calling `record_workload_baseline`.

- [x] **Step 3: Include comparison JSON and fail when requested**

Include `baseline_comparison` in output when comparison is requested. Return an error when `--fail-on-regression` is set and the comparison does not pass.

- [x] **Step 4: Verify GREEN**

Run: `cargo test -p continuitydb-cli cli_measure_workload_compares_baseline`

Expected: PASS.

### Task 3: Docs, Full Gate, Commit

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-workload-baseline-cli-gate.md`

- [x] **Step 1: Update docs**

Record the CLI workload baseline regression gate in README current scope and Benchmark and Workload milestones.

- [x] **Step 2: Run full verification**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

- [x] **Step 3: Commit**

Commit with `feat: gate workload baseline regressions from cli`.
