# Commit Materialization API Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `ContinuityDb::commit_cells` to hydrate the ordered StateCells written by one commit boundary.

**Architecture:** Compose existing kernel primitives in `continuitydb-api`: lookup a `CommitManifest`, return `CommitNotFound` when absent, then hydrate `manifest.cell_ids` through the existing ordered cell lookup helper. No storage-kernel changes are needed.

**Tech Stack:** Rust 2021, `continuitydb-api`, `CommitId`, `CommitManifest`, `StateCell`, existing `KernelError`.

---

## File Structure

- `crates/continuitydb-api/src/lib.rs`: Add tests and `commit_cells`.
- `README.md`: Add current scope bullet.
- `docs/roadmap.md`: Add native API milestone.
- `docs/superpowers/plans/2026-05-20-commit-materialization-api.md`: Track execution.

### Task 1: API Commit Cell Materialization

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`

- [x] **Step 1: Write failing tests**

Add these tests in `crates/continuitydb-api/src/lib.rs`:

```rust
#[test]
fn api_returns_commit_cells_in_manifest_order() -> Result<(), Box<dyn std::error::Error>> {
    let committed_at = Utc
        .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let commit_id = CommitId::new();
    let mut db = ContinuityDb::new(MemoryKernel::default());
    let first = sample_cell("project:continuitydb:commit-cells-first", 0.91, 12)?;
    let second = sample_cell("project:continuitydb:commit-cells-second", 0.83, 15)?;
    let expected_ids = vec![first.id, second.id];

    db.ingest_cells_at_with_commit_id(vec![first, second], committed_at, commit_id)?;

    let cells = db.commit_cells(commit_id)?;
    assert_eq!(
        cells.iter().map(|cell| cell.id).collect::<Vec<_>>(),
        expected_ids
    );
    assert!(cells.iter().all(|cell| cell.commit_id == commit_id));
    Ok(())
}

#[test]
fn api_commit_cells_reports_unknown_commit() {
    let db = ContinuityDb::new(MemoryKernel::default());

    let result = db.commit_cells(CommitId::new());

    assert!(matches!(
        result,
        Err(ContinuityError::Kernel(KernelError::CommitNotFound))
    ));
}
```

- [x] **Step 2: Run tests to verify they fail**

Run: `cargo test -p continuitydb-api commit_cells`

Expected: FAIL because `ContinuityDb::commit_cells` does not exist.

- [x] **Step 3: Implement `commit_cells`**

Add in the `impl<K: StorageKernel> ContinuityDb<K>` block near commit manifest methods:

```rust
/// Returns StateCells written by a database commit in manifest order.
pub fn commit_cells(&self, commit_id: CommitId) -> Result<Vec<StateCell>, ContinuityError> {
    let manifest = self
        .commit_manifest(commit_id)?
        .ok_or(ContinuityError::Kernel(KernelError::CommitNotFound))?;
    self.lookup_cells_in_order(manifest.cell_ids)
}
```

- [x] **Step 4: Run tests to verify they pass**

Run: `cargo test -p continuitydb-api commit_cells`

Expected: PASS.

### Task 2: Documentation and Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-commit-materialization-api.md`

- [x] **Step 1: Update docs**

In `README.md`, add:

```markdown
- Native commit cell materialization API.
```

In `docs/roadmap.md`, add Native API milestone 5:

```markdown
5. Add ordered commit cell materialization. Implemented `ContinuityDb::commit_cells` so callers can hydrate the StateCells written by one commit in manifest order, with unknown commits reported as `CommitNotFound`.
```

- [x] **Step 2: Run focused test**

Run: `cargo test -p continuitydb-api commit_cells`

Expected: PASS.

- [x] **Step 3: Run full verification gate**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

Expected: all PASS.

- [x] **Step 4: Commit**

Run:

```bash
git add crates/continuitydb-api/src/lib.rs README.md docs/roadmap.md docs/superpowers/plans/2026-05-20-commit-materialization-api.md
git commit -m "feat: add commit cell materialization"
```
