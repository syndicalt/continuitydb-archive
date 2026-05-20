# Native Commit Export Batches Design

## Goal

Expose a typed, deterministic commit export batch from the native API so embedders can build backup, sync, and replay flows without reassembling commit manifests and cells themselves.

## Current Behavior

`ContinuityDb::commit_slices` already materializes cursor-selected commit manifests with their ordered StateCells. Callers still need to infer export metadata, especially whether another cursor should be used and which commit was last included.

## Proposed Behavior

Add:

```rust
pub struct CommitExportBatch {
    pub slices: Vec<CommitSlice>,
    pub next_after: Option<CommitId>,
}
```

Add:

```rust
pub fn export_commits(&self, lookup: CommitManifestLookup) -> Result<CommitExportBatch, ContinuityError>
```

Semantics:

- `slices` is identical to `commit_slices(lookup)`.
- `next_after` is the last exported commit ID when at least one slice is returned.
- `next_after` is `None` for an empty export.
- Unknown cursors propagate `KernelError::CommitNotFound`.
- Empty databases return an empty batch with `next_after: None`.

This is deliberately a typed native API, not a JSON wire format. A future CLI or sync crate can serialize this type once the replay/import side exists.

## Non-Goals

- Do not add import/replay in this slice.
- Do not add a CLI export command yet.
- Do not change `StorageKernel`.
- Do not add serialization derives unless a wire format is introduced.

## Tests

Add API tests proving:

- Export batches preserve commit order and cell order.
- Export batches return `next_after` equal to the last exported commit.
- Unknown cursors are reported.
- Empty databases export an empty batch.
