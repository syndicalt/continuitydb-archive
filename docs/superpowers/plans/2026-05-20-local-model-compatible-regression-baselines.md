# Local Model Compatible Regression Baselines Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make local-model benchmark regression gates compare only compatible prior baselines.

**Architecture:** Add a compatibility predicate over candidate identity, response schema version, and runtime manifest. Use it in a new compatible-baseline lookup helper and switch the record-and-compare gate to build the current baseline first, compare against the latest compatible prior baseline, then append the current baseline.

**Tech Stack:** Rust 2021, `continuitydb-steward` with `local-model`.

---

### Task 1: RED Tests

**Files:**
- Modify: `crates/continuitydb-steward/src/lib.rs`

- [x] **Step 1: Add incompatible runtime gate test**

Add `local_model_baseline_gate_skips_incompatible_runtime_baseline`. It should store a passing baseline for the same candidate but with a different runner argument than the current benchmark. The current benchmark should fail the suite. Assert `report.regression() == None`, `!report.regressed()`, and the store contains both baselines.

- [x] **Step 2: Keep compatible regression test meaningful**

Update `local_model_baseline_gate_reports_regression_against_latest_previous` so previous and current benchmark runs use the same runtime manifest while producing different model responses.

- [x] **Step 3: Run RED focused tests**

Run:

```bash
cargo test -p continuitydb-steward local_model_baseline_gate_skips_incompatible_runtime_baseline --features local-model
cargo test -p continuitydb-steward local_model_baseline_gate_reports_regression_against_latest_previous --features local-model
```

Expected: the incompatible runtime test fails before implementation because candidate-only lookup still reports a regression.

### Task 2: GREEN Implementation

**Files:**
- Modify: `crates/continuitydb-steward/src/local_model.rs`

- [x] **Step 1: Add compatibility lookup helper**

Add `latest_compatible_local_model_benchmark_baseline(store, current)` that filters by candidate ID, candidate role, response schema version, and runtime manifest before selecting the newest `recorded_at`.

- [x] **Step 2: Switch record-and-compare gate**

Change `record_local_model_benchmark_baseline_with_regression` to:

1. run benchmark into a current baseline
2. find latest compatible previous baseline
3. compare if present
4. append current baseline
5. return `LocalModelBenchmarkGateReport`

- [x] **Step 3: Run GREEN focused tests**

Run the focused tests from Task 1. Expected: PASS.

### Task 3: Docs and Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-local-model-compatible-regression-baselines.md`

- [x] **Step 1: Update README scope**

Add a scope bullet for compatible local-model benchmark regression gates.

- [x] **Step 2: Update roadmap**

Add a Steward milestone noting that local-model regression gates compare only compatible runtime/schema baselines.

- [x] **Step 3: Run full verification gate**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

Expected: PASS.

- [x] **Step 4: Mark plan complete**

Check off all completed boxes in this plan.

- [x] **Step 5: Commit**

Commit with:

```bash
git add README.md docs/roadmap.md docs/superpowers/specs/2026-05-20-local-model-compatible-regression-baselines-design.md docs/superpowers/plans/2026-05-20-local-model-compatible-regression-baselines.md crates/continuitydb-steward/src/local_model.rs crates/continuitydb-steward/src/lib.rs
git commit -m "feat: compare compatible local model baselines"
```
