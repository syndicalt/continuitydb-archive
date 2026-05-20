# Workload Residual Lookup-Plan Regression Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make workload baseline comparison detect changes in residual exact lookup-plan constraints.

**Architecture:** Extend the workload regression enum with a residual exact constraints variant. Reuse the existing ordered vector comparison helper in `push_lookup_plan_regressions`.

**Tech Stack:** Rust workspace, `continuitydb-workload`, serde enum JSON, cargo tests.

---

### Task 1: Detect residual exact constraint drift

**Files:**
- Modify: `crates/continuitydb-workload/src/lib.rs`
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [x] **Step 1: Write failing test**

Add:

```rust
fn workload_baseline_regression_detects_lookup_plan_residual_constraint_change()
```

The test should compare a baseline plan with `residual_exact_constraints: []` against a current plan with `residual_exact_constraints: ["valid_at"]`, and expect:

```rust
WorkloadBaselineRegression::LookupPlanResidualExactConstraintsChanged {
    previous: Vec::new(),
    current: vec!["valid_at".to_string()],
}
```

- [x] **Step 2: Run focused test to verify RED**

Run:

```sh
cargo test -p continuitydb-workload workload_baseline_regression_detects_lookup_plan_residual_constraint_change
```

Expected: compile or assertion failure because the regression variant is not implemented.

- [x] **Step 3: Implement minimal production code**

Add the enum variant:

```rust
LookupPlanResidualExactConstraintsChanged {
    previous: Vec<String>,
    current: Vec<String>,
}
```

In `push_lookup_plan_regressions`, call `push_if_changed` for `residual_exact_constraints`.

- [x] **Step 4: Run focused test to verify GREEN**

Run:

```sh
cargo test -p continuitydb-workload workload_baseline_regression_detects_lookup_plan_residual_constraint_change
```

Expected: pass.

- [x] **Step 5: Update docs**

Record residual lookup-plan regression detection in `README.md` and `docs/roadmap.md`.

- [x] **Step 6: Run verification gates**

Run:

```sh
cargo fmt --all
cargo fmt --all -- --check
git diff --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
```

- [x] **Step 7: Commit**

Commit:

```sh
git add README.md crates/continuitydb-workload/src/lib.rs docs/roadmap.md docs/superpowers/plans/2026-05-20-workload-residual-lookup-plan-regression.md docs/superpowers/specs/2026-05-20-workload-residual-lookup-plan-regression-design.md
git commit -m "feat: detect residual lookup-plan regressions"
```
