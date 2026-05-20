# CLI Workload Baseline Recording Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Allow `continuitydb measure-workload` to append JSONL baseline records while still printing measurement JSON.

**Architecture:** Reuse `FileWorkloadBaselineStore`, `WorkloadBaselineRecord`, and `WorkloadMeasurementSnapshot` from `continuitydb-workload`. The CLI keeps existing measurement behavior when no baseline path is supplied and appends one baseline record when `--baseline-path` is present.

**Tech Stack:** Rust 2021, clap, serde_json, `continuitydb-workload`, existing CLI smoke tests.

---

### Task 1: RED CLI Baseline Tests

**Files:**
- Modify: `crates/continuitydb-cli/tests/cli.rs`

- [x] **Step 1: Add failing CLI tests**

Add tests for memory and file workload measurement with `--baseline-path` and `--label`, asserting that one JSONL baseline record is written with the expected kernel, label, and snapshot counts.

- [x] **Step 2: Verify RED**

Run: `cargo test -p continuitydb-cli measure_workload_records_baseline`

Expected: FAIL because `--baseline-path` and `--label` are not accepted.

### Task 2: GREEN CLI Baseline Recording

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`

- [x] **Step 1: Implement baseline options and append logic**

Add command fields for `baseline_path` and `label`. After measuring, append a baseline record when a path is supplied and include baseline metadata in JSON output.

- [x] **Step 2: Verify GREEN**

Run: `cargo test -p continuitydb-cli measure_workload_records_baseline`

Expected: PASS.

### Task 3: Docs, Full Gate, Commit

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-cli-workload-baseline-recording.md`

- [x] **Step 1: Update docs**

Record CLI workload baseline recording in README current scope and Benchmark and Workload milestones.

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

Commit with `feat: record workload baselines from cli`.
