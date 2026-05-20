# Workload Measurement Baselines Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a durable JSONL baseline store for deterministic workload measurement records.

**Architecture:** Extend `continuitydb-workload` with serializable measurement snapshots and a file-backed append/list baseline store. The store is append-only and independent of the CLI so future regression gates can reuse it directly.

**Tech Stack:** Rust 2021, `continuitydb-workload`, `serde`, `serde_json`, std file I/O.

---

### Task 1: RED Baseline Store Tests

**Files:**
- Modify: `crates/continuitydb-workload/Cargo.toml`
- Modify: `crates/continuitydb-workload/src/lib.rs`

- [x] **Step 1: Add serde dependencies and failing tests**

Add `serde` and `serde_json` dependencies. Add tests for snapshot conversion, baseline append/list order, missing file behavior, and corrupt JSONL line diagnostics.

- [x] **Step 2: Verify RED**

Run: `cargo test -p continuitydb-workload workload_baseline`

Expected: FAIL because snapshot and baseline store types do not exist.

### Task 2: GREEN Baseline Store

**Files:**
- Modify: `crates/continuitydb-workload/src/lib.rs`

- [x] **Step 1: Implement snapshots, records, errors, and file store**

Implement `MeasuredOperationSnapshot`, `CheckoutMeasurementSnapshot`, `WorkloadMeasurementSnapshot`, `WorkloadBaselineRecord`, `WorkloadBaselineError`, and `FileWorkloadBaselineStore`.

- [x] **Step 2: Verify GREEN**

Run: `cargo test -p continuitydb-workload workload_baseline`

Expected: PASS.

### Task 3: Docs, Full Gate, Commit

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-workload-measurement-baselines.md`

- [x] **Step 1: Update docs**

Record workload measurement baseline storage in README current scope and Benchmark and Workload milestones.

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

Commit with `feat: record workload measurement baselines`.
