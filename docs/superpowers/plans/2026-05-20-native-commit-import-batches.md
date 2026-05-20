# Native Commit Import Batches Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a validated native commit import API that replays exported commit batches into another ContinuityDB store.

**Architecture:** Build on `CommitExportBatch` and existing kernel commit-stamped append APIs. Validate the entire batch before mutating the target kernel, then append each slice with its original commit ID and commit time.

**Tech Stack:** Rust, existing `continuitydb-api`, `continuitydb-memory`, `continuitydb-kernel::CellLookup`.

---

## File Structure

- Modify `crates/continuitydb-api/src/lib.rs`: add `ContinuityError::InvalidCommitExport`, `ContinuityDb::import_commit_batch`, helper validation, and API tests.
- Modify `README.md`: add native commit import batch API to current scope.
- Modify `docs/roadmap.md`: add Native API milestone.

## Task 1: Failing Import Batch Tests

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`

- [ ] **Step 1: Add successful import test**

Add this test near the export batch tests:

```rust
#[test]
fn api_imports_exported_commit_batch() -> Result<(), Box<dyn std::error::Error>> {
    let first_time = Utc
        .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let second_time = Utc
        .with_ymd_and_hms(2026, 5, 20, 12, 30, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let first_commit = CommitId::new();
    let second_commit = CommitId::new();
    let mut source = ContinuityDb::new(MemoryKernel::default());
    source.ingest_cells_at_with_commit_id(
        vec![sample_cell("project:continuitydb:import-first", 0.91, 12)?],
        first_time,
        first_commit,
    )?;
    source.ingest_cells_at_with_commit_id(
        vec![
            sample_cell("project:continuitydb:import-second-a", 0.83, 15)?,
            sample_cell("project:continuitydb:import-second-b", 0.82, 16)?,
        ],
        second_time,
        second_commit,
    )?;
    let batch = source.export_commits(CommitManifestLookup::default())?;
    let mut target = ContinuityDb::new(MemoryKernel::default());

    let imported = target.import_commit_batch(batch.clone())?;

    assert_eq!(imported, 2);
    assert_eq!(target.export_commits(CommitManifestLookup::default())?, batch);
    Ok(())
}
```

- [ ] **Step 2: Add empty import test**

Add:

```rust
#[test]
fn api_imports_empty_commit_batch_without_mutation() -> Result<(), Box<dyn std::error::Error>> {
    let mut target = ContinuityDb::new(MemoryKernel::default());

    let imported = target.import_commit_batch(CommitExportBatch {
        slices: Vec::new(),
        next_after: None,
    })?;

    assert_eq!(imported, 0);
    assert!(target.commit_slices(CommitManifestLookup::default())?.is_empty());
    Ok(())
}
```

- [ ] **Step 3: Add malformed batch test**

Add:

```rust
#[test]
fn api_import_rejects_malformed_batch_without_mutation(
) -> Result<(), Box<dyn std::error::Error>> {
    let committed_at = Utc
        .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let commit_id = CommitId::new();
    let mut cell = sample_cell("project:continuitydb:malformed-import", 0.91, 12)?;
    cell.commit_id = commit_id;
    cell.system_time = continuitydb_core::SystemTimeRange::open_from(committed_at);
    let manifest = continuitydb_core::CommitManifest::new(commit_id, committed_at, vec![StateCellId::new()]);
    let mut target = ContinuityDb::new(MemoryKernel::default());

    let result = target.import_commit_batch(CommitExportBatch {
        slices: vec![CommitSlice {
            manifest,
            cells: vec![cell],
        }],
        next_after: Some(commit_id),
    });

    assert!(matches!(
        result,
        Err(ContinuityError::InvalidCommitExport { commit_id: rejected }) if rejected == commit_id
    ));
    assert!(target.commit_slices(CommitManifestLookup::default())?.is_empty());
    Ok(())
}
```

- [ ] **Step 4: Add existing commit rejection test**

Add:

```rust
#[test]
fn api_import_rejects_existing_target_commit_without_mutation(
) -> Result<(), Box<dyn std::error::Error>> {
    let committed_at = Utc
        .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let commit_id = CommitId::new();
    let mut source = ContinuityDb::new(MemoryKernel::default());
    source.ingest_cells_at_with_commit_id(
        vec![sample_cell("project:continuitydb:existing-source", 0.91, 12)?],
        committed_at,
        commit_id,
    )?;
    let batch = source.export_commits(CommitManifestLookup::default())?;
    let mut target = ContinuityDb::new(MemoryKernel::default());
    target.ingest_cells_at_with_commit_id(
        vec![sample_cell("project:continuitydb:existing-target", 0.83, 15)?],
        committed_at,
        commit_id,
    )?;

    let result = target.import_commit_batch(batch);

    assert!(matches!(
        result,
        Err(ContinuityError::Kernel(KernelError::DuplicateCommit))
    ));
    assert_eq!(target.commit_slices(CommitManifestLookup::default())?.len(), 1);
    Ok(())
}
```

- [ ] **Step 5: Run targeted tests and verify RED**

Run:

```bash
cargo test -p continuitydb-api import_
```

Expected: compilation fails because `import_commit_batch` and `InvalidCommitExport` do not exist yet.

## Task 2: Import Batch Implementation

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`

- [ ] **Step 1: Add `HashSet` import**

Add near the top:

```rust
use std::collections::HashSet;
```

- [ ] **Step 2: Add error variant**

Add to `ContinuityError`:

```rust
/// Commit export batch failed deterministic validation.
#[error("commit export batch is invalid for commit {commit_id}")]
InvalidCommitExport {
    /// Commit whose export slice failed validation.
    commit_id: CommitId,
},
```

- [ ] **Step 3: Add import method**

Add near `export_commits`:

```rust
/// Imports a validated commit export batch into the backing kernel.
pub fn import_commit_batch(
    &mut self,
    batch: CommitExportBatch,
) -> Result<usize, ContinuityError> {
    self.validate_commit_export_batch(&batch)?;
    let imported = batch.slices.len();
    for slice in batch.slices {
        self.kernel.append_cells_at_with_commit_id(
            slice.cells,
            slice.manifest.committed_at,
            slice.manifest.commit_id,
        )?;
    }
    Ok(imported)
}
```

- [ ] **Step 4: Add validation helper**

Add below `lookup_one_cell`:

```rust
fn validate_commit_export_batch(
    &self,
    batch: &CommitExportBatch,
) -> Result<(), ContinuityError> {
    let mut commit_ids = HashSet::new();
    let mut cell_ids = HashSet::new();
    for slice in &batch.slices {
        let commit_id = slice.manifest.commit_id;
        if !commit_ids.insert(commit_id) {
            return Err(ContinuityError::InvalidCommitExport { commit_id });
        }
        if self.commit_manifest(commit_id)?.is_some() {
            return Err(ContinuityError::Kernel(KernelError::DuplicateCommit));
        }

        let exported_ids = slice.cells.iter().map(|cell| cell.id).collect::<Vec<_>>();
        if exported_ids != slice.manifest.cell_ids {
            return Err(ContinuityError::InvalidCommitExport { commit_id });
        }
        for cell in &slice.cells {
            if cell.commit_id != commit_id
                || cell.system_time.from() != slice.manifest.committed_at
            {
                return Err(ContinuityError::InvalidCommitExport { commit_id });
            }
            if !cell_ids.insert(cell.id) {
                return Err(ContinuityError::InvalidCommitExport { commit_id });
            }
            if !self
                .kernel
                .lookup_cells(CellLookup {
                    cell_id: Some(cell.id),
                    ..CellLookup::default()
                })?
                .is_empty()
            {
                return Err(ContinuityError::Kernel(KernelError::DuplicateCell));
            }
        }
    }
    Ok(())
}
```

- [ ] **Step 5: Run targeted tests and verify GREEN**

Run:

```bash
cargo test -p continuitydb-api import_
```

Expected: all import batch tests pass.

## Task 3: Docs and Full Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-native-commit-import-batches.md`

- [ ] **Step 1: Update README**

Add to Current Scope:

```markdown
- Native validated commit import batch API for replay flows.
```

- [ ] **Step 2: Update roadmap**

Add Native API milestone:

```markdown
9. Add native validated commit import batches. Implemented `ContinuityDb::import_commit_batch` so exported commit batches can be validated and replayed into another store while preserving commit IDs, commit times, cell IDs, and manifest ordering.
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
git add crates/continuitydb-api/src/lib.rs README.md docs/roadmap.md docs/superpowers/specs/2026-05-20-native-commit-import-batches-design.md docs/superpowers/plans/2026-05-20-native-commit-import-batches.md
git commit -m "feat: add native commit import batches"
```
