# File Lookup Plan Selectivity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add deterministic candidate selectivity diagnostics to file-kernel lookup plans and workload regression gates.

**Architecture:** Keep lookup planning and exact filtering unchanged. Derive `candidate_selectivity_basis_points` from existing candidate and exact match counts, then serialize it through CLI JSON and workload snapshots and compare it in workload baselines.

**Tech Stack:** Rust workspace, `continuitydb-kernel`, `continuitydb-cli`, `continuitydb-workload`, serde JSON, cargo tests.

---

### Task 1: Add lookup-plan candidate selectivity

**Files:**
- Modify: `crates/continuitydb-kernel/src/lib.rs`
- Modify: `crates/continuitydb-cli/src/main.rs`
- Modify: `crates/continuitydb-cli/tests/cli.rs`
- Modify: `crates/continuitydb-workload/src/lib.rs`
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [x] **Step 1: Write failing tests**

Add tests that expect `candidate_selectivity_basis_points` in kernel plans, CLI lookup-plan JSON, workload snapshots, and workload baseline regression comparison.

- [x] **Step 2: Run focused tests to verify RED**

Run:

```sh
cargo test -p continuitydb-kernel file_kernel_lookup_plan_reports_candidate_selectivity
cargo test -p continuitydb-cli cli_inspect_kernel_reports_lookup_plan_candidate_selectivity
cargo test -p continuitydb-workload workload_snapshot_preserves_lookup_plan_candidate_selectivity
cargo test -p continuitydb-workload workload_baseline_regression_detects_lookup_plan_candidate_selectivity_change
```

Expected: failures because `candidate_selectivity_basis_points` is not present yet.

- [x] **Step 3: Implement minimal production code**

Add `candidate_selectivity_basis_points` to `FileKernelLookupPlan`, compute it as `exact_match_count * 10_000 / candidate_count` with zero-candidate protection, include it in CLI JSON, preserve it in `WorkloadLookupPlanSnapshot`, and compare it in workload baseline regressions.

- [x] **Step 4: Run focused tests to verify GREEN**

Run the same focused tests and confirm they pass.

- [x] **Step 5: Update docs**

Record the selectivity lookup-plan milestone in `README.md` and `docs/roadmap.md`.

- [x] **Step 6: Run verification gates**

Run:

```sh
cargo fmt --all -- --check
git diff --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
```

- [x] **Step 7: Commit**

Commit:

```sh
git add README.md crates/continuitydb-kernel/src/lib.rs crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs crates/continuitydb-workload/src/lib.rs docs/roadmap.md docs/superpowers/plans/2026-05-20-file-lookup-plan-selectivity.md docs/superpowers/specs/2026-05-20-file-lookup-plan-selectivity-design.md
git commit -m "feat: report lookup-plan candidate selectivity"
```
