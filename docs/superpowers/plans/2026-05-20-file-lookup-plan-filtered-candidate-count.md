# File Lookup Plan Filtered Candidate Count Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add filtered candidate counts to file-kernel lookup-plan diagnostics and workload regression gates.

**Architecture:** Keep the existing planner and exact matching logic unchanged. Derive `filtered_candidate_count` from `candidate_count` and `exact_match_count`, then serialize it through CLI JSON and workload baseline snapshots.

**Tech Stack:** Rust workspace, `continuitydb-kernel`, `continuitydb-cli`, `continuitydb-workload`, serde JSON, cargo tests.

---

### Task 1: Add filtered candidate counts to lookup plans

**Files:**
- Modify: `crates/continuitydb-kernel/src/lib.rs`
- Modify: `crates/continuitydb-cli/src/main.rs`
- Modify: `crates/continuitydb-cli/tests/cli.rs`
- Modify: `crates/continuitydb-workload/src/lib.rs`
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [x] **Step 1: Write failing tests**

Add tests that expect `filtered_candidate_count` in kernel plans, CLI lookup-plan JSON, workload snapshots, and workload baseline regression comparison.

- [x] **Step 2: Run focused tests to verify RED**

Run:

```sh
cargo test -p continuitydb-kernel file_kernel_lookup_plan_reports_filtered_candidate_count
cargo test -p continuitydb-cli cli_inspect_kernel_reports_lookup_plan_filtered_candidate_count
cargo test -p continuitydb-workload workload_snapshot_preserves_lookup_plan_filtered_candidate_count
cargo test -p continuitydb-workload workload_baseline_regression_detects_lookup_plan_filtered_candidate_count_change
```

Expected: failures because `filtered_candidate_count` is not present yet.

- [x] **Step 3: Implement minimal production code**

Add `filtered_candidate_count` to `FileKernelLookupPlan`, compute it from `candidate_count - exact_match_count`, include it in CLI JSON, preserve it in `WorkloadLookupPlanSnapshot`, and compare it in workload baseline regressions.

- [x] **Step 4: Run focused tests to verify GREEN**

Run the same focused tests and confirm they pass.

- [x] **Step 5: Update docs**

Record the filtered-candidate lookup-plan milestone in `README.md` and `docs/roadmap.md`.

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
git add README.md crates/continuitydb-kernel/src/lib.rs crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs crates/continuitydb-workload/src/lib.rs docs/roadmap.md docs/superpowers/plans/2026-05-20-file-lookup-plan-filtered-candidate-count.md docs/superpowers/specs/2026-05-20-file-lookup-plan-filtered-candidate-count-design.md
git commit -m "feat: report lookup-plan filtered candidates"
```
