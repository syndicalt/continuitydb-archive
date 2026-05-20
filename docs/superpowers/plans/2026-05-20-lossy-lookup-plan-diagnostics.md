# Lossy Lookup Plan Diagnostics Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add lookup-plan diagnostics that identify indexed constraints known to over-select and require exact residual filtering.

**Architecture:** Extend `FileKernelLookupPlan` with lossy indexed constraint labels derived from the current `CellLookup`. Propagate the fields through CLI JSON and workload snapshots so benchmark artifacts can preserve and compare the planner signal.

**Tech Stack:** Rust workspace, `continuitydb-kernel`, `continuitydb-cli`, `continuitydb-workload`, serde JSON, cargo tests.

---

### Task 1: Add lossy indexed constraint diagnostics

**Files:**
- Modify: `crates/continuitydb-kernel/src/lib.rs`
- Modify: `crates/continuitydb-cli/src/main.rs`
- Modify: `crates/continuitydb-cli/tests/cli.rs`
- Modify: `crates/continuitydb-workload/src/lib.rs`
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [x] **Step 1: Write failing kernel, CLI, and workload tests**

Add assertions that lookup plans expose:

```rust
lossy_indexed_constraint_count: 2
lossy_indexed_constraints: ["system_at", "valid_at"]
```

for lookup requests containing temporal as-of constraints.

- [x] **Step 2: Run focused tests to verify RED**

Run:

```sh
cargo test -p continuitydb-kernel file_kernel_lookup_plan_reports_lossy_indexed_constraints
cargo test -p continuitydb-cli cli_inspect_kernel_reports_lookup_plan_lossy_indexed_constraints
cargo test -p continuitydb-workload workload_snapshot_preserves_lookup_plan_lossy_indexed_constraints
```

Expected: failures because the new fields do not exist yet.

- [x] **Step 3: Implement minimal production code**

Add the two fields to `FileKernelLookupPlan`, derive labels from `CellLookup.system_at` and `CellLookup.valid_at`, emit the fields from CLI JSON, and persist them in workload snapshots.

- [x] **Step 4: Run focused tests to verify GREEN**

Run the same focused tests and confirm they pass.

- [x] **Step 5: Update docs**

Record the milestone in `README.md` and `docs/roadmap.md`.

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
git add README.md crates/continuitydb-kernel/src/lib.rs crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs crates/continuitydb-workload/src/lib.rs docs/roadmap.md docs/superpowers/plans/2026-05-20-lossy-lookup-plan-diagnostics.md docs/superpowers/specs/2026-05-20-lossy-lookup-plan-diagnostics-design.md
git commit -m "feat: report lookup-plan lossy constraints"
```
