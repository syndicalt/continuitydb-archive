# Commit Manifest Design

## Goal

Add first-class commit manifests so ContinuityDB can answer "what happened in this database commit?" without reconstructing the answer from stamped StateCells alone.

## Context

ContinuityDB already assigns `CommitId` values to appended StateCells and supports commit-scoped lookup and checkout. That proves transaction-scoped materialization, but it leaves the commit boundary implicit. A production datastore needs an explicit manifest for audit, replication, commit inspection, and future durable commit logs.

## Design

Introduce `CommitManifest` in `continuitydb-core` as a small immutable domain object:

- `commit_id: CommitId`
- `committed_at: DateTime<Utc>`
- `cell_ids: Vec<StateCellId>`

The manifest is intentionally minimal. It records the durable commit boundary and ordered cells written by that boundary. It does not yet include actor identity, operation type, policy metadata, checksums, or parent commits; those belong to later audit and replication slices once the first manifest contract is stable.

## Kernel Contract

Extend `StorageKernel` with:

- `lookup_commit_manifest(commit_id: CommitId) -> Result<Option<CommitManifest>, KernelError>`

Existing append methods keep their public shape. Kernels create the manifest internally when `append_cells_at_with_commit_id` succeeds with a non-empty batch. The manifest's `cell_ids` order must match append order. Empty batches remain no-ops and must not create manifests.

Duplicate cell rejection must stay atomic: if any cell in a batch is invalid or duplicate, no cells and no manifest become visible.

## Memory Kernel

`MemoryKernel` stores manifests in append order in memory. It should reject a second non-empty write with an already-visible `CommitId`, because one commit ID must identify one immutable commit boundary.

## File Kernel

The first durable implementation can reconstruct manifests from the existing JSONL cell log at open time by grouping cells by `commit_id` and preserving append order. New successful appends update the in-memory manifest index after the cell records are written.

This avoids a file-format migration for the first manifest slice. A later storage-engine milestone can add explicit mixed record types or a separate commit log with stronger crash recovery semantics.

## API

Expose:

- `ContinuityDb::commit_manifest(commit_id: CommitId) -> Result<Option<CommitManifest>, ContinuityError>`

This keeps commit inspection on the native embeddable API while preserving the storage kernel as the source of commit visibility.

## Roadmap Updates

Update the storage roadmap to include first-class commit manifests after commit identifiers. Update the README current scope once the implementation is complete.

## Tests

Add failing tests before implementation:

- Core manifest preserves commit ID, timestamp, and ordered cell IDs.
- Memory kernel creates a manifest for successful commit-stamped batches.
- Memory kernel rejects duplicate commit IDs without partial visibility.
- File kernel reconstructs manifests after reopen.
- API returns a manifest for a committed batch and `None` for an unknown commit.

Run the full verification gate before committing implementation:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

## Out of Scope

- Commit actor identity.
- Operation metadata.
- Merkle checksums.
- Parent commit DAGs.
- Explicit on-disk manifest records.
- Replication or distributed consensus.
