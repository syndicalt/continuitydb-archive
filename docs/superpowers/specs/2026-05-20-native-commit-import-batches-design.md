# Native Commit Import Batches Design

## Goal

Add a typed native API that replays `CommitExportBatch` values into another store while preserving commit IDs, commit times, StateCell IDs, cell order, and manifest boundaries.

## Current Behavior

`ContinuityDb::export_commits` returns ordered commit slices plus a next cursor for backup and sync reads. There is no corresponding native replay/import path, so embedders must write their own manifest and cell validation before appending to a target database.

## Proposed Behavior

Add:

```rust
pub fn import_commit_batch(&mut self, batch: CommitExportBatch) -> Result<usize, ContinuityError>
```

The method returns the number of imported commit slices.

Before mutating the target kernel, validate the full batch:

- each slice's cell IDs exactly match `manifest.cell_ids`;
- each cell's `commit_id` matches the slice manifest commit ID;
- each cell's `system_time.from()` matches `manifest.committed_at`;
- no duplicate commit IDs appear in the batch;
- none of the commit IDs already exist in the target;
- none of the StateCell IDs already exist in the target.

If validation fails, return a deterministic `ContinuityError` and do not mutate the target. After validation, append each slice through the existing kernel `append_cells_at_with_commit_id` boundary, preserving the commit ID and committed time.

## Error Model

Add:

```rust
ContinuityError::InvalidCommitExport { commit_id: CommitId }
```

Use it for structural export-batch mismatches and duplicate commit IDs inside the batch. Use existing kernel errors for target conflicts:

- existing target commit: `KernelError::DuplicateCommit`
- existing target StateCell ID: `KernelError::DuplicateCell`

## Non-Goals

- Do not add JSON serialization or a CLI import command in this slice.
- Do not implement cross-store deduplication or merge policies.
- Do not make the multi-slice import physically atomic across I/O failures.
- Do not change `StorageKernel`.

## Tests

Add API tests proving:

- exporting from one memory store and importing into another preserves commit slices;
- importing an empty batch returns zero and leaves the target empty;
- malformed batches are rejected without mutating the target;
- existing target commits are rejected before mutation.
