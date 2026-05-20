# Local Model Baseline Gate Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a single API that records a current local model benchmark baseline and compares it against the latest previous baseline for the same candidate when available.

**Architecture:** Reuse the existing recorder, latest-baseline lookup, and regression comparison primitives. The new report stores the current baseline and an optional regression report, where `None` means no previous baseline existed before the current run.

**Tech Stack:** Rust 2021, `continuitydb-steward` with the `local-model` feature.

---

### Task 1: RED Gate Tests

**Files:**
- Modify: `crates/continuitydb-steward/src/lib.rs`

- [x] **Step 1: Write failing tests**

Add feature-gated tests near the local model baseline tests:
- `local_model_baseline_gate_records_without_previous_regression`
- `local_model_baseline_gate_reports_regression_against_latest_previous`

The first test should run the gate against an empty `MemoryLocalModelBenchmarkBaselineStore`, assert the returned current baseline is stored, and assert `regression() == None`.

The second test should pre-seed the store with a passing baseline for `small_model_candidates()[0]`, run a current failing benchmark for the same candidate, and assert the returned regression is `Some` and `regressed() == true`.

- [x] **Step 2: Verify RED**

Run:

```bash
cargo test -p continuitydb-steward --features local-model baseline_gate
```

Expected: compilation fails because `record_local_model_benchmark_baseline_with_regression` and `LocalModelBenchmarkGateReport` are not implemented/exported.

### Task 2: Implement Gate API

**Files:**
- Modify: `crates/continuitydb-steward/src/local_model.rs`
- Modify: `crates/continuitydb-steward/src/lib.rs`

- [x] **Step 1: Add gate report**

Add `LocalModelBenchmarkGateReport` with:
- `current_baseline: LocalModelBenchmarkBaseline`
- `regression: Option<LocalModelBenchmarkRegression>`

Expose accessors:
- `current_baseline(&self) -> &LocalModelBenchmarkBaseline`
- `regression(&self) -> Option<&LocalModelBenchmarkRegression>`
- `regressed(&self) -> bool`

- [x] **Step 2: Add record-and-compare function**

Add `record_local_model_benchmark_baseline_with_regression` that:
- Finds `previous` with `latest_local_model_benchmark_baseline(store, benchmark.candidate())?` before recording.
- Records `current` with `record_local_model_benchmark_baseline(...)`.
- Builds `regression` with `previous.as_ref().map(|previous| LocalModelBenchmarkRegression::compare(previous, &current))`.
- Returns `LocalModelBenchmarkGateReport`.

- [x] **Step 3: Export the API**

Export `LocalModelBenchmarkGateReport` and `record_local_model_benchmark_baseline_with_regression` from `lib.rs`, and import them in tests.

- [x] **Step 4: Verify GREEN**

Run:

```bash
cargo test -p continuitydb-steward --features local-model baseline_gate
```

Expected: both gate tests pass.

### Task 3: Roadmap and Verification

**Files:**
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-local-model-baseline-gate.md`

- [x] **Step 1: Update roadmap milestone 6**

Update milestone 6 to include a record-and-regression baseline gate.

- [x] **Step 2: Mark this plan complete**

Check off completed steps in this plan before commit.

- [x] **Step 3: Verify**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

Expected: all checks pass.
