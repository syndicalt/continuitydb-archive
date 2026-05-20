# Native Commit Export Batches Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a native typed commit export batch API for backup, sync, and replay preparation.

**Architecture:** Build on the existing `commit_slices` API. Add a small `CommitExportBatch` type with exported slices and the cursor callers should pass as `after` for the next batch.

**Tech Stack:** Rust, existing `continuitydb-api`, `continuitydb-kernel::CommitManifestLookup`, `continuitydb-memory` tests.

---

## File Structure

- Modify `crates/continuitydb-api/src/lib.rs`: add `CommitExportBatch`, `ContinuityDb::export_commits`, and API tests.
- Modify `README.md`: add native commit export batch API to current scope.
- Modify `docs/roadmap.md`: add Native API milestone.

## Task 1: Failing Export Batch Tests

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`

- [x] **Step 1: Import new type in tests**

Change:

```rust
use super::{ContinuityDb, ContinuityError};
```

to:

```rust
use super::{CommitExportBatch, ContinuityDb, ContinuityError};
```

- [x] **Step 2: Add export batch order and cursor test**

Add this test near `api_returns_commit_slices_after_cursor_with_limit`:

```rust
#[test]
fn api_exports_commit_batch_with_next_cursor() -> Result<(), Box<dyn std::error::Error>> {
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
    let mut db = ContinuityDb::new(MemoryKernel::default());
    let first = sample_cell("project:continuitydb:export-first", 0.91, 12)?;
    let second_a = sample_cell("project:continuitydb:export-second-a", 0.83, 15)?;
    let second_b = sample_cell("project:continuitydb:export-second-b", 0.82, 16)?;
    let expected_second_ids = vec![second_a.id, second_b.id];

    db.ingest_cells_at_with_commit_id(vec![first], first_time, first_commit)?;
    db.ingest_cells_at_with_commit_id(vec![second_a, second_b], second_time, second_commit)?;

    let batch = db.export_commits(CommitManifestLookup {
        after: Some(first_commit),
        limit: Some(1),
    })?;

    assert_eq!(batch.next_after, Some(second_commit));
    assert_eq!(batch.slices.len(), 1);
    assert_eq!(batch.slices[0].manifest.commit_id, second_commit);
    assert_eq!(
        batch.slices[0]
            .cells
            .iter()
            .map(|cell| cell.id)
            .collect::<Vec<_>>(),
        expected_second_ids
    );
    Ok(())
}
```

- [x] **Step 3: Add empty export test**

Add:

```rust
#[test]
fn api_exports_empty_commit_batch() -> Result<(), Box<dyn std::error::Error>> {
    let db = ContinuityDb::new(MemoryKernel::default());

    let batch = db.export_commits(CommitManifestLookup::default())?;

    assert_eq!(
        batch,
        CommitExportBatch {
            slices: Vec::new(),
            next_after: None,
        }
    );
    Ok(())
}
```

- [x] **Step 4: Add unknown cursor test**

Add:

```rust
#[test]
fn api_export_commits_reports_unknown_cursor() {
    let db = ContinuityDb::new(MemoryKernel::default());

    let result = db.export_commits(CommitManifestLookup {
        after: Some(CommitId::new()),
        limit: Some(10),
    });

    assert!(matches!(
        result,
        Err(ContinuityError::Kernel(KernelError::CommitNotFound))
    ));
}
```

- [x] **Step 5: Run targeted tests and verify RED**

Run:

```bash
cargo test -p continuitydb-api export_commits
```

Expected: compilation fails because `CommitExportBatch` and `export_commits` do not exist yet.

## Task 2: Export Batch Implementation

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`

- [x] **Step 1: Add export batch type**

Add below `CommitSlice`:

```rust
/// Cursor-selected commit slices ready for backup, sync, or replay export.
#[derive(Clone, Debug, PartialEq)]
pub struct CommitExportBatch {
    /// Exported commit slices in database visibility order.
    pub slices: Vec<CommitSlice>,
    /// Cursor to use as `CommitManifestLookup.after` for the next export batch.
    pub next_after: Option<CommitId>,
}
```

- [x] **Step 2: Add export method**

Add near `commit_slices`:

```rust
/// Returns a cursor-selected commit export batch for backup, sync, and replay flows.
pub fn export_commits(
    &self,
    lookup: CommitManifestLookup,
) -> Result<CommitExportBatch, ContinuityError> {
    let slices = self.commit_slices(lookup)?;
    let next_after = slices.last().map(|slice| slice.manifest.commit_id);
    Ok(CommitExportBatch { slices, next_after })
}
```

- [x] **Step 3: Run targeted tests and verify GREEN**

Run:

```bash
cargo test -p continuitydb-api export_commits
cargo test -p continuitydb-api exports_
```

Expected: all export batch tests pass.

## Task 3: Docs and Full Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-native-commit-export-batches.md`

- [x] **Step 1: Update README**

Add to Current Scope:

```markdown
- Native commit export batch API for backup and sync flows.
```

- [x] **Step 2: Update roadmap**

Add Native API milestone:

```markdown
8. Add native commit export batches. Implemented `CommitExportBatch` and `ContinuityDb::export_commits` so embedders can page commit slices with a deterministic next cursor for backup, sync, and replay flows.
```

- [x] **Step 3: Run full verification gate**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

Expected: every command exits 0.

- [x] **Step 4: Commit**

Run:

```bash
git add crates/continuitydb-api/src/lib.rs README.md docs/roadmap.md docs/superpowers/specs/2026-05-20-native-commit-export-batches-design.md docs/superpowers/plans/2026-05-20-native-commit-export-batches.md
git commit -m "feat: add native commit export batches"
```
