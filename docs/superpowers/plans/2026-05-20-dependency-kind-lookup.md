# Dependency Kind Lookup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make dependency kind a standalone indexed StateCell lookup constraint.

**Architecture:** Preserve the existing `CellLookup` API and extend its semantics so dependency target and dependency kind are independently useful constraints. The file kernel gets a secondary dependency-kind index that participates in candidate selection and lookup-plan diagnostics, while exact filtering remains the final authority.

**Tech Stack:** Rust workspace, `continuitydb-kernel`, `continuitydb-memory`, cargo tests, existing JSONL file kernel indexes.

---

### Task 1: Add dependency-kind-only storage semantics

**Files:**
- Modify: `crates/continuitydb-memory/src/lib.rs`
- Modify: `crates/continuitydb-kernel/src/lib.rs`
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [x] **Step 1: Write failing tests**

Add tests for memory lookup, file lookup after reopen, and file lookup-plan diagnostics using `CellLookup { dependency_kind: Some(CellDependencyKind::DependsOn), dependency_target: None, .. }`.

- [x] **Step 2: Run focused tests to verify RED**

Run:

```sh
cargo test -p continuitydb-memory memory_kernel_filters_by_dependency_kind_without_target
cargo test -p continuitydb-kernel file_kernel_filters_by_dependency_kind_without_target
cargo test -p continuitydb-kernel file_kernel_lookup_plan_reports_dependency_kind_index
```

Expected: at least the kind-only lookup tests fail because `dependency_kind` is ignored without `dependency_target`.

- [x] **Step 3: Implement minimal production code**

Update memory and file exact filtering so dependency constraints match when either `dependency_target` or `dependency_kind` is present. Add a `dependency_kinds` index to `FileKernelIndex`, populate it on insert, and include it in `indexed_candidate_constraints` when kind is present without target.

- [x] **Step 4: Run focused tests to verify GREEN**

Run the same focused tests and confirm they pass.

- [x] **Step 5: Update docs**

Record the dependency-kind standalone lookup milestone in `README.md` and `docs/roadmap.md`.

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
git add README.md crates/continuitydb-kernel/src/lib.rs crates/continuitydb-memory/src/lib.rs docs/roadmap.md docs/superpowers/plans/2026-05-20-dependency-kind-lookup.md docs/superpowers/specs/2026-05-20-dependency-kind-lookup-design.md
git commit -m "feat: index dependency-kind lookups"
```
