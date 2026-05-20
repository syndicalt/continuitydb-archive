# Workload Baseline Regression Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add deterministic comparison between current workload measurement snapshots and the latest matching baseline record.

**Architecture:** Extend `continuitydb-workload` with latest matching baseline lookup and a pure comparison function. Count fields are exact invariants; elapsed times use an explicit tolerance percentage.

**Tech Stack:** Rust 2021, `continuitydb-workload`, existing JSONL baseline store tests.

---

### Task 1: RED Regression Tests

**Files:**
- Modify: `crates/continuitydb-workload/src/lib.rs`

- [x] **Step 1: Add failing tests**

Add tests for latest matching baseline lookup, equal snapshot pass, count mismatch failure, and elapsed tolerance behavior.

- [x] **Step 2: Verify RED**

Run: `cargo test -p continuitydb-workload workload_baseline_regression`

Expected: FAIL because latest matching and comparison types/functions do not exist.

### Task 2: GREEN Regression Comparator

**Files:**
- Modify: `crates/continuitydb-workload/src/lib.rs`

- [x] **Step 1: Implement latest matching lookup and comparison types**

Implement `FileWorkloadBaselineStore::latest_matching`, `WorkloadBaselineComparison`, `WorkloadBaselineRegression`, and `compare_workload_snapshot_to_baseline`.

- [x] **Step 2: Verify GREEN**

Run: `cargo test -p continuitydb-workload workload_baseline_regression`

Expected: PASS.

### Task 3: Docs, Full Gate, Commit

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-workload-baseline-regression.md`

- [x] **Step 1: Update docs**

Record workload baseline regression comparison in README current scope and Benchmark and Workload milestones.

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

Commit with `feat: compare workload baseline regressions`.
