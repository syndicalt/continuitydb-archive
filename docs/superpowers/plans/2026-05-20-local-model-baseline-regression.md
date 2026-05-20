# Local Model Baseline Regression Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add deterministic comparison of local model benchmark baselines so stored results can act as a regression gate.

**Architecture:** Compare two `LocalModelBenchmarkBaseline` records for the same candidate. The report should expose previous/current pass counts, pass-count delta, candidate identity, and whether the current run regressed.

**Tech Stack:** Rust 2021, `continuitydb-steward` with the `local-model` feature.

---

### Task 1: RED Regression Tests

**Files:**
- Modify: `crates/continuitydb-steward/src/lib.rs`

- [x] **Step 1: Write failing tests**

Add feature-gated tests near the local model baseline tests:
- `local_model_baseline_regression_detects_pass_count_drop`
- `local_model_baseline_regression_allows_equal_quality`

The first test should create a previous passing baseline and a current failing baseline for `small_model_candidates()[0]`, call `LocalModelBenchmarkRegression::compare(&previous, &current)`, and assert:
- `regressed() == true`
- `previous_passed_cases() == 1`
- `current_passed_cases() == 0`
- `pass_count_delta() == -1`

The second test should compare two passing baselines for the same candidate and assert:
- `regressed() == false`
- `pass_count_delta() == 0`

- [x] **Step 2: Verify RED**

Run:

```bash
cargo test -p continuitydb-steward --features local-model baseline_regression
```

Expected: compilation fails because `LocalModelBenchmarkRegression` is not implemented/exported.

### Task 2: Implement Regression Report

**Files:**
- Modify: `crates/continuitydb-steward/src/local_model.rs`
- Modify: `crates/continuitydb-steward/src/lib.rs`

- [x] **Step 1: Add regression report type**

Add `LocalModelBenchmarkRegression` after `LocalModelBenchmarkBaseline`. It should include:
- candidate model id
- candidate role
- previous recorded timestamp
- current recorded timestamp
- previous passed case count
- current passed case count
- pass count delta
- regressed boolean

Regression rule:

```rust
let regressed = current_passed_cases < previous_passed_cases
    || (previous.passed() && !current.passed());
```

- [x] **Step 2: Add accessors and export**

Add accessors for all report fields. Export `LocalModelBenchmarkRegression` from `lib.rs` and import it in tests.

- [x] **Step 3: Verify GREEN**

Run:

```bash
cargo test -p continuitydb-steward --features local-model baseline_regression
```

Expected: both regression tests pass.

### Task 3: Roadmap and Verification

**Files:**
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-local-model-baseline-regression.md`

- [x] **Step 1: Update roadmap milestone 6**

Update milestone 6 to include deterministic regression comparison for recorded local model benchmark baselines.

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
