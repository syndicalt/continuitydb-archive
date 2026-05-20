# StateCell Dependencies Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add first-class dependency and causal references to `StateCell` so cells can express what they depend on, derive from, support, or are caused by.

**Architecture:** Keep dependencies in `continuitydb-core` as serializable semantic metadata on the cell itself. Use `StateCellId` references and a compact `CellDependencyKind` enum, defaulting to an empty dependency list for new and older serialized cells.

**Tech Stack:** Rust 2021, serde, existing `continuitydb-core` domain types.

---

### Task 1: Core Dependency Primitive

**Files:**
- Modify: `crates/continuitydb-core/src/cell.rs`
- Modify: `crates/continuitydb-core/src/lib.rs`
- Modify: `docs/roadmap.md`
- Create: `docs/superpowers/plans/2026-05-20-state-cell-dependencies.md`

- [x] **Step 1: Write the failing tests**

Add tests in `crates/continuitydb-core/src/lib.rs`:

```rust
#[test]
fn cell_dependency_records_target_kind_and_rationale() {
    let target = StateCellId::new();
    let dependency = CellDependency::new(
        target,
        CellDependencyKind::DependsOn,
        "release status depends on verification evidence",
    );

    assert_eq!(dependency.target, target);
    assert_eq!(dependency.kind, CellDependencyKind::DependsOn);
    assert_eq!(
        dependency.rationale,
        "release status depends on verification evidence"
    );
}

#[test]
fn state_cell_starts_with_empty_dependencies() -> Result<(), Box<dyn std::error::Error>> {
    let cell = sample_state_cell("project:continuitydb:dependencies")?;

    assert!(cell.dependencies.is_empty());
    Ok(())
}

#[test]
fn state_cell_deserializes_missing_dependencies_as_empty(
) -> Result<(), Box<dyn std::error::Error>> {
    let cell = sample_state_cell("project:continuitydb:legacy-dependencies")?;
    let mut value = serde_json::to_value(cell)?;
    value
        .as_object_mut()
        .ok_or_else(|| std::io::Error::other("state cell did not serialize as object"))?
        .remove("dependencies");

    let decoded: StateCell = serde_json::from_value(value)?;

    assert!(decoded.dependencies.is_empty());
    Ok(())
}
```

Extract a small `sample_state_cell(anchor: &str)` helper if needed to avoid duplicating fixture construction.

- [x] **Step 2: Run test to verify it fails**

Run: `cargo test -p continuitydb-core dependencies`

Expected: FAIL because `CellDependency`, `CellDependencyKind`, and `StateCell::dependencies` do not exist.

- [x] **Step 3: Implement dependency types**

Add to `cell.rs`:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CellDependencyKind {
    DependsOn,
    CausedBy,
    Supports,
    DerivedFrom,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CellDependency {
    pub target: StateCellId,
    pub kind: CellDependencyKind,
    pub rationale: String,
}

impl CellDependency {
    pub fn new(target: StateCellId, kind: CellDependencyKind, rationale: impl Into<String>) -> Self {
        Self { target, kind, rationale: rationale.into() }
    }
}
```

Add `#[serde(default)] pub dependencies: Vec<CellDependency>` to `StateCell`, initialize it to `Vec::new()` in `StateCell::new`, and re-export both new types from `continuitydb-core`.

- [x] **Step 4: Run focused tests**

Run:

```bash
cargo test -p continuitydb-core dependencies
cargo test -p continuitydb-core
```

Expected: PASS.

- [x] **Step 5: Update roadmap**

Add a new dependency/causality milestone section noting first-class StateCell dependency references.

- [x] **Step 6: Run workspace verification**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

Expected: all commands pass.

- [x] **Step 7: Commit**

```bash
git add crates/continuitydb-core/src/cell.rs crates/continuitydb-core/src/lib.rs docs/roadmap.md docs/superpowers/plans/2026-05-20-state-cell-dependencies.md
git commit -m "feat: add state cell dependencies"
```
