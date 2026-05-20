# File Lookup Plan Exact Match Count Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add exact post-filter match counts to file-kernel lookup-plan diagnostics.

**Architecture:** Keep existing candidate planning unchanged. Add an `exact_match_count` field computed by applying the same lookup predicate used by `lookup_cells` to the candidate set, then serialize that field through CLI and workload snapshot artifacts.

**Tech Stack:** Rust workspace, `continuitydb-kernel`, `continuitydb-cli`, `continuitydb-workload`, serde JSON, cargo tests.

---

### Task 1: Add exact match counts to lookup plans

**Files:**
- Modify: `crates/continuitydb-kernel/src/lib.rs`
- Modify: `crates/continuitydb-cli/src/main.rs`
- Modify: `crates/continuitydb-cli/tests/cli.rs`
- Modify: `crates/continuitydb-workload/src/lib.rs`
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [x] **Step 1: Write failing tests**

Add tests that expect `exact_match_count` in kernel plans, CLI lookup-plan JSON, and workload snapshots.

- [x] **Step 2: Run focused tests to verify RED**

Run:

```sh
cargo test -p continuitydb-kernel file_kernel_lookup_plan_reports_exact_match_count_after_filtering
cargo test -p continuitydb-cli cli_inspect_kernel_reports_lookup_plan_exact_match_count
cargo test -p continuitydb-workload workload_snapshot_preserves_lookup_plan_exact_match_count
```

Expected: failures because `exact_match_count` is not present yet.

- [x] **Step 3: Implement minimal production code**

Add `exact_match_count` to `FileKernelLookupPlan`, compute it from candidate positions filtered by exact lookup semantics, include it in CLI JSON, and preserve it in `WorkloadLookupPlanSnapshot`.

- [x] **Step 4: Run focused tests to verify GREEN**

Run the same focused tests and confirm they pass.

- [x] **Step 5: Update docs**

Record the exact-match lookup-plan milestone in `README.md` and `docs/roadmap.md`.

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
git add README.md crates/continuitydb-kernel/src/lib.rs crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs crates/continuitydb-workload/src/lib.rs docs/roadmap.md docs/superpowers/plans/2026-05-20-file-lookup-plan-exact-match-count.md docs/superpowers/specs/2026-05-20-file-lookup-plan-exact-match-count-design.md
git commit -m "feat: report lookup-plan exact match counts"
```
