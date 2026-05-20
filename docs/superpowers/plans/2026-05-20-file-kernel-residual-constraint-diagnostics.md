# File Kernel Residual Constraint Diagnostics Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Expose exact lookup constraints that still require residual filtering after indexed candidate selection in file-kernel lookup plans.

**Architecture:** Extend `FileKernelLookupPlan` with ordered residual exact constraint fields. Compute them from the existing exact constraint list, indexed constraint list, and lossy indexed constraint list, then surface the data through CLI JSON and workload snapshots without changing lookup behavior.

**Tech Stack:** Rust workspace, `continuitydb-kernel`, `continuitydb-cli`, `continuitydb-workload`, serde JSON, assert_cmd.

---

### Task 1: Add residual lookup-plan diagnostics

**Files:**
- Modify: `crates/continuitydb-kernel/src/lib.rs`
- Modify: `crates/continuitydb-cli/src/main.rs`
- Modify: `crates/continuitydb-cli/tests/cli.rs`
- Modify: `crates/continuitydb-workload/src/lib.rs`
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [x] **Step 1: Write failing tests**

Add tests:

```rust
fn file_kernel_lookup_plan_reports_residual_exact_constraints()
fn cli_inspect_kernel_reports_lookup_plan_residual_exact_constraints()
fn workload_snapshot_preserves_lookup_plan_residual_exact_constraints()
```

Expected behavior:

```rust
residual_exact_constraint_count == 1
residual_exact_constraints == ["valid_at"]
```

- [x] **Step 2: Run focused tests to verify RED**

Run:

```sh
cargo test -p continuitydb-kernel file_kernel_lookup_plan_reports_residual_exact_constraints
cargo test -p continuitydb-cli cli_inspect_kernel_reports_lookup_plan_residual_exact_constraints
cargo test -p continuitydb-workload workload_snapshot_preserves_lookup_plan_residual_exact_constraints
```

Expected: compilation or assertion failures because the new fields do not exist yet.

- [x] **Step 3: Implement minimal production code**

Add fields to `FileKernelLookupPlan`:

```rust
pub residual_exact_constraint_count: usize,
pub residual_exact_constraints: Vec<&'static str>,
```

In `lookup_plan`, compute:

```rust
let residual_exact_constraints = exact_constraints
    .iter()
    .copied()
    .filter(|constraint| {
        !indexed_constraints.contains(constraint)
            || lossy_indexed_constraints.contains(constraint)
    })
    .collect::<Vec<_>>();
```

Then populate the new count and list. Add matching JSON fields in `file_lookup_plan_json` and string fields in `WorkloadLookupPlanSnapshot`.

- [x] **Step 4: Run focused tests to verify GREEN**

Run:

```sh
cargo test -p continuitydb-kernel file_kernel_lookup_plan_reports_residual_exact_constraints
cargo test -p continuitydb-cli cli_inspect_kernel_reports_lookup_plan_residual_exact_constraints
cargo test -p continuitydb-workload workload_snapshot_preserves_lookup_plan_residual_exact_constraints
```

Expected: all pass.

- [x] **Step 5: Update docs**

Record residual lookup-plan diagnostics in `README.md` and `docs/roadmap.md`.

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
git add README.md crates/continuitydb-kernel/src/lib.rs crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs crates/continuitydb-workload/src/lib.rs docs/roadmap.md docs/superpowers/plans/2026-05-20-file-kernel-residual-constraint-diagnostics.md docs/superpowers/specs/2026-05-20-file-kernel-residual-constraint-diagnostics-design.md
git commit -m "feat: report residual lookup constraints"
```
