# CLI Workload Measurement Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a CLI command that runs deterministic workload measurement against memory or file-backed kernels and emits JSON.

**Architecture:** Reuse `continuitydb-workload` from `continuitydb-cli`. The CLI builds a stable workload config and checkout request, selects a memory or file kernel, runs `measure_ingest_and_checkout`, and serializes measurement counts plus elapsed nanoseconds.

**Tech Stack:** Rust 2021, clap, serde_json, `continuitydb-workload`, existing CLI smoke tests.

---

### Task 1: RED CLI Tests

**Files:**
- Modify: `crates/continuitydb-cli/Cargo.toml`
- Modify: `crates/continuitydb-cli/tests/cli.rs`

- [x] **Step 1: Add failing CLI tests**

Add tests for `measure-workload --kernel memory`, `measure-workload --kernel file --store-path <path>`, and missing file store path.

- [x] **Step 2: Verify RED**

Run: `cargo test -p continuitydb-cli measure_workload`

Expected: FAIL because the command does not exist.

### Task 2: GREEN CLI Command

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`

- [x] **Step 1: Implement command, kernel selector, and JSON output**

Add `MeasureWorkload` command handling, a `WorkloadKernelProfile` value enum, workload config/request helpers, and measurement JSON helpers.

- [x] **Step 2: Verify GREEN**

Run: `cargo test -p continuitydb-cli measure_workload`

Expected: PASS.

### Task 3: Docs, Full Gate, Commit

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-cli-workload-measurement.md`

- [x] **Step 1: Update docs**

Record CLI workload measurement in README current scope and Benchmark and Workload milestones.

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

Commit with `feat: add workload measurement cli`.
