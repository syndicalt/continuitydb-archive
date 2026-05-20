# Dependency Lookup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make StateCell dependency metadata operational by adding deterministic storage-kernel lookup filters for dependency target and dependency kind.

**Architecture:** Extend `CellLookup` in `continuitydb-kernel` with optional dependency filters. Apply identical semantics in `FileKernel` and `MemoryKernel`: a cell matches when any dependency points to the requested target and, if supplied, has the requested dependency kind.

**Tech Stack:** Rust 2021, `continuitydb-core`, `continuitydb-kernel`, `continuitydb-memory`, serde-backed file kernel tests.

---

### Task 1: Dependency Lookup Filters

**Files:**
- Modify: `crates/continuitydb-kernel/src/lib.rs`
- Modify: `crates/continuitydb-memory/src/lib.rs`
- Modify: `docs/roadmap.md`
- Create: `docs/superpowers/plans/2026-05-20-dependency-lookup.md`

- [x] **Step 1: Write failing tests**

Add `file_kernel_filters_by_dependency_target_and_kind` in `crates/continuitydb-kernel/src/lib.rs` and `memory_kernel_filters_by_dependency_target_and_kind` in `crates/continuitydb-memory/src/lib.rs`.

Each test should:

```rust
let target = StateCellId::new();
let other_target = StateCellId::new();
let mut dependent = sample_cell("project:continuitydb:dependent", 0.9, 12)?;
dependent.dependencies.push(CellDependency::new(
    target,
    CellDependencyKind::DependsOn,
    "depends on target",
));
let mut unrelated = sample_cell("project:continuitydb:unrelated", 0.9, 12)?;
unrelated.dependencies.push(CellDependency::new(
    other_target,
    CellDependencyKind::DependsOn,
    "depends on different target",
));
let mut support = sample_cell("project:continuitydb:support", 0.9, 12)?;
support.dependencies.push(CellDependency::new(
    target,
    CellDependencyKind::Supports,
    "supports target",
));
```

Append all three cells. Lookup with `dependency_target: Some(target)` and `dependency_kind: Some(CellDependencyKind::DependsOn)`. Assert only `dependent` is returned.

- [x] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p continuitydb-kernel dependency_target
cargo test -p continuitydb-memory dependency_target
```

Expected: FAIL because `CellLookup::dependency_target` and `CellLookup::dependency_kind` do not exist.

- [x] **Step 3: Implement lookup fields**

Add to `CellLookup`:

```rust
pub dependency_target: Option<StateCellId>,
pub dependency_kind: Option<CellDependencyKind>,
```

Import `CellDependencyKind` and `StateCellId` into `continuitydb-kernel`.

- [x] **Step 4: Implement file and memory filters**

Add a helper in both file and memory lookup chains:

```rust
.filter(|cell| {
    lookup.dependency_target.map_or(true, |target| {
        cell.dependencies.iter().any(|dependency| {
            dependency.target == target
                && lookup
                    .dependency_kind
                    .map_or(true, |kind| dependency.kind == kind)
        })
    })
})
```

- [x] **Step 5: Run focused tests**

Run:

```bash
cargo test -p continuitydb-kernel dependency_target
cargo test -p continuitydb-memory dependency_target
```

Expected: PASS.

- [x] **Step 6: Update roadmap**

Add a dependency/causality milestone for dependency-aware storage lookup across memory and file kernels.

- [x] **Step 7: Run workspace verification**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

Expected: all commands pass.

- [x] **Step 8: Commit**

```bash
git add crates/continuitydb-kernel/src/lib.rs crates/continuitydb-memory/src/lib.rs docs/roadmap.md docs/superpowers/plans/2026-05-20-dependency-lookup.md
git commit -m "feat: filter cells by dependencies"
```
