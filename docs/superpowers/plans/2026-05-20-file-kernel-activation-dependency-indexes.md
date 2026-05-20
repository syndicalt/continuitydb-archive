# File Kernel Activation and Dependency Indexes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add deterministic in-process activation-state and dependency target/kind indexes to the durable file kernel.

**Architecture:** Extend `FileKernelIndex` with derived indexes for activation states, dependency targets, and dependency target/kind pairs. Populate them through the existing `insert` path so indexes rebuild from the JSONL log on open and update after durable append. Keep final lookup predicates unchanged for correctness across combined filters.

**Tech Stack:** Rust, existing `continuitydb-core` derives, existing `continuitydb-kernel` tests, no new dependencies.

---

## File Structure

- Modify `crates/continuitydb-core/src/cell.rs`: derive `Hash` for `ActivationState` and `CellDependencyKind`.
- Modify `crates/continuitydb-kernel/src/lib.rs`: add index fields, update insert and lookup candidate selection, add tests.
- Modify `README.md`: add activation/dependency file-kernel indexes to current scope.
- Modify `docs/roadmap.md`: add Storage Kernel milestone.
- Modify `docs/superpowers/plans/2026-05-20-file-kernel-activation-dependency-indexes.md`: track completed steps.

## Task 1: Failing Activation and Dependency Index Tests

**Files:**
- Modify: `crates/continuitydb-kernel/src/lib.rs`

- [ ] **Step 1: Add activation rebuild test**

Add near `file_kernel_filters_by_activation_state`:

```rust
#[test]
fn file_kernel_rebuilds_activation_index() -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_kernel_path("continuitydb-file-kernel-activation-index-rebuild");
    let mut active = sample_cell("project:continuitydb:index-active", 0.91, 12)?;
    active.activation = ActivationState::Active;
    let mut frontier = sample_cell("project:continuitydb:index-frontier-activation", 0.83, 15)?;
    frontier.activation = ActivationState::Frontier;
    {
        let mut kernel = FileKernel::open(&path)?;
        append_committed(&mut kernel, active)?;
        frontier = append_committed(&mut kernel, frontier)?;
    }

    let reopened = FileKernel::open(&path)?;
    let positions = reopened
        .index
        .activations
        .get(&ActivationState::Frontier)
        .cloned()
        .unwrap_or_default();
    let indexed = positions
        .iter()
        .map(|position| reopened.index.cells[*position].clone())
        .collect::<Vec<_>>();

    assert_eq!(indexed, vec![frontier]);
    fs::remove_file(path)?;
    Ok(())
}
```

- [ ] **Step 2: Add activation append-update test**

Add:

```rust
#[test]
fn file_kernel_updates_activation_index_after_append() -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_kernel_path("continuitydb-file-kernel-activation-index-append");
    let mut frontier = sample_cell("project:continuitydb:index-append-frontier-activation", 0.83, 15)?;
    frontier.activation = ActivationState::Frontier;
    let mut kernel = FileKernel::open(&path)?;

    frontier = append_committed(&mut kernel, frontier)?;
    let positions = kernel
        .index
        .activations
        .get(&ActivationState::Frontier)
        .cloned()
        .unwrap_or_default();
    let indexed = positions
        .iter()
        .map(|position| kernel.index.cells[*position].clone())
        .collect::<Vec<_>>();

    assert_eq!(indexed, vec![frontier]);
    fs::remove_file(path)?;
    Ok(())
}
```

- [ ] **Step 3: Add dependency rebuild test**

Add near `file_kernel_filters_by_dependency_target_and_kind`:

```rust
#[test]
fn file_kernel_rebuilds_dependency_indexes() -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_kernel_path("continuitydb-file-kernel-dependency-index-rebuild");
    let target = StateCellId::new();
    let other_target = StateCellId::new();
    let mut dependent = sample_cell("project:continuitydb:index-dependent", 0.9, 12)?;
    dependent.dependencies.push(CellDependency::new(
        target,
        CellDependencyKind::DependsOn,
        "depends on target",
    ));
    let mut support = sample_cell("project:continuitydb:index-support", 0.9, 12)?;
    support.dependencies.push(CellDependency::new(
        target,
        CellDependencyKind::Supports,
        "supports target",
    ));
    let mut unrelated = sample_cell("project:continuitydb:index-unrelated", 0.9, 12)?;
    unrelated.dependencies.push(CellDependency::new(
        other_target,
        CellDependencyKind::DependsOn,
        "depends on other target",
    ));
    {
        let mut kernel = FileKernel::open(&path)?;
        dependent = append_committed(&mut kernel, dependent)?;
        support = append_committed(&mut kernel, support)?;
        append_committed(&mut kernel, unrelated)?;
    }

    let reopened = FileKernel::open(&path)?;
    let target_positions = reopened
        .index
        .dependency_targets
        .get(&target)
        .cloned()
        .unwrap_or_default();
    let target_indexed = target_positions
        .iter()
        .map(|position| reopened.index.cells[*position].clone())
        .collect::<Vec<_>>();
    let kind_positions = reopened
        .index
        .dependency_target_kinds
        .get(&(target, CellDependencyKind::DependsOn))
        .cloned()
        .unwrap_or_default();
    let kind_indexed = kind_positions
        .iter()
        .map(|position| reopened.index.cells[*position].clone())
        .collect::<Vec<_>>();

    assert_eq!(target_indexed, vec![dependent.clone(), support]);
    assert_eq!(kind_indexed, vec![dependent]);
    fs::remove_file(path)?;
    Ok(())
}
```

- [ ] **Step 4: Add dependency append-update test**

Add:

```rust
#[test]
fn file_kernel_updates_dependency_indexes_after_append() -> Result<(), Box<dyn std::error::Error>>
{
    let path = temp_kernel_path("continuitydb-file-kernel-dependency-index-append");
    let target = StateCellId::new();
    let mut dependent = sample_cell("project:continuitydb:index-append-dependent", 0.9, 12)?;
    dependent.dependencies.push(CellDependency::new(
        target,
        CellDependencyKind::DependsOn,
        "depends on target",
    ));
    let mut kernel = FileKernel::open(&path)?;

    dependent = append_committed(&mut kernel, dependent)?;
    let target_positions = kernel
        .index
        .dependency_targets
        .get(&target)
        .cloned()
        .unwrap_or_default();
    let target_indexed = target_positions
        .iter()
        .map(|position| kernel.index.cells[*position].clone())
        .collect::<Vec<_>>();
    let kind_positions = kernel
        .index
        .dependency_target_kinds
        .get(&(target, CellDependencyKind::DependsOn))
        .cloned()
        .unwrap_or_default();
    let kind_indexed = kind_positions
        .iter()
        .map(|position| kernel.index.cells[*position].clone())
        .collect::<Vec<_>>();

    assert_eq!(target_indexed, vec![dependent.clone()]);
    assert_eq!(kind_indexed, vec![dependent]);
    fs::remove_file(path)?;
    Ok(())
}
```

- [ ] **Step 5: Run targeted tests and verify RED**

Run:

```bash
cargo test -p continuitydb-kernel file_kernel_rebuilds_activation_index
cargo test -p continuitydb-kernel file_kernel_rebuilds_dependency_indexes
```

Expected: compilation fails because `activations`, `dependency_targets`, and `dependency_target_kinds` are not fields on `FileKernelIndex`.

## Task 2: Activation and Dependency Index Implementation

**Files:**
- Modify: `crates/continuitydb-core/src/cell.rs`
- Modify: `crates/continuitydb-kernel/src/lib.rs`

- [ ] **Step 1: Derive Hash for index key types**

Change:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ActivationState {
```

to:

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum ActivationState {
```

Change:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CellDependencyKind {
```

to:

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum CellDependencyKind {
```

- [ ] **Step 2: Add index fields**

Change `FileKernelIndex` to include:

```rust
activations: HashMap<ActivationState, Vec<usize>>,
dependency_targets: HashMap<StateCellId, Vec<usize>>,
dependency_target_kinds: HashMap<(StateCellId, CellDependencyKind), Vec<usize>>,
```

- [ ] **Step 3: Populate indexes during insert**

In `FileKernelIndex::insert`, after evidence-source indexing and before commit indexing, add:

```rust
self.activations
    .entry(cell.activation)
    .or_default()
    .push(position);
for dependency in &cell.dependencies {
    self.dependency_targets
        .entry(dependency.target)
        .or_default()
        .push(position);
    self.dependency_target_kinds
        .entry((dependency.target, dependency.kind))
        .or_default()
        .push(position);
}
```

- [ ] **Step 4: Use indexes for candidate selection**

In `FileKernel::lookup_cells`, add activation/dependency branches after evidence-source lookup and before full scan:

```rust
} else if let Some(activation) = lookup.activation {
    self.index
        .activations
        .get(&activation)
        .map(|positions| {
            positions
                .iter()
                .map(|position| &self.index.cells[*position])
                .collect()
        })
        .unwrap_or_default()
} else if let Some(target) = lookup.dependency_target {
    if let Some(kind) = lookup.dependency_kind {
        self.index
            .dependency_target_kinds
            .get(&(target, kind))
            .map(|positions| {
                positions
                    .iter()
                    .map(|position| &self.index.cells[*position])
                    .collect()
            })
            .unwrap_or_default()
    } else {
        self.index
            .dependency_targets
            .get(&target)
            .map(|positions| {
                positions
                    .iter()
                    .map(|position| &self.index.cells[*position])
                    .collect()
            })
            .unwrap_or_default()
    }
```

- [ ] **Step 5: Run targeted tests and verify GREEN**

Run:

```bash
cargo test -p continuitydb-kernel file_kernel_rebuilds_activation_index
cargo test -p continuitydb-kernel file_kernel_updates_activation_index_after_append
cargo test -p continuitydb-kernel file_kernel_rebuilds_dependency_indexes
cargo test -p continuitydb-kernel file_kernel_updates_dependency_indexes_after_append
cargo test -p continuitydb-kernel file_kernel_filters_by_activation_state
cargo test -p continuitydb-kernel file_kernel_filters_by_dependency_target_and_kind
```

Expected: all targeted index and lookup tests pass.

## Task 3: Docs and Full Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-file-kernel-activation-dependency-indexes.md`

- [ ] **Step 1: Update README**

Add to Current Scope near the secondary file-kernel index bullet:

```markdown
- File-kernel secondary indexes for activation states and dependency filters.
```

- [ ] **Step 2: Update roadmap**

Add a Storage Kernel milestone after the answerability/evidence index milestone:

```markdown
22. Add file-kernel secondary indexes for activation and dependency lookups. Implemented derived in-process indexes for activation states, dependency targets, and dependency target/kind pairs so frontier and causality filters can start from indexed candidates while preserving append-order results.
```

- [ ] **Step 3: Run full verification gate**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

Expected: every command exits 0.

- [ ] **Step 4: Commit**

Run:

```bash
git add README.md docs/roadmap.md crates/continuitydb-core/src/cell.rs crates/continuitydb-kernel/src/lib.rs docs/superpowers/plans/2026-05-20-file-kernel-activation-dependency-indexes.md
git commit -m "feat: add file kernel activation dependency indexes"
```
