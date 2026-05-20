# File Kernel Commit Records Design

## Goal

Make the durable `FileKernel` persist explicit commit manifest records instead of relying only on manifest reconstruction from StateCell rows.

## Context

The file kernel currently stores one raw serialized `StateCell` per JSONL line. On reopen, it rebuilds commit manifests by grouping cells by `StateCell.commit_id` in file order. That preserves current behavior, but the commit boundary itself is implicit. A production datastore needs the commit boundary to be a durable record because audit, replay, backup, and future format migration should be able to inspect commits without inferring them from individual cells.

## Design

Introduce an internal JSONL envelope for new file-kernel writes:

```json
{"type":"cell","cell":{...StateCell...}}
{"type":"commit","manifest":{...CommitManifest...}}
```

New non-empty batches should append all cell records first, followed by one commit record. Existing raw `StateCell` lines must remain readable for backward compatibility. Reopen should use explicit commit records as the authoritative manifest when present, while legacy raw-cell-only logs continue to reconstruct manifests from cells.

The storage-kernel trait does not change. This is an internal durable format improvement for `FileKernel`.

## Ordering and Atomicity

For a successful non-empty batch:

1. Validate duplicate cell IDs and duplicate commit IDs before writing.
2. Stamp all cells with the shared system transaction time and commit ID.
3. Build `CommitManifest::new(commit_id, committed_at, ordered_cell_ids)`.
4. Append one JSONL payload containing all cell records followed by the commit record.
5. Update the in-memory index only after the file write succeeds.

This preserves the existing no-partial-visibility behavior for duplicate validation and in-memory state. The current JSONL file kernel does not provide fsync-level crash recovery; that remains out of scope.

## Reopen Semantics

`FileKernel::open` should parse each non-empty line as either:

- a new envelope record with `type = "cell"` or `type = "commit"`;
- or a legacy raw `StateCell`.

When explicit commit records exist:

- The manifest record supplies committed time, cell order, and listing order.
- The record is corrupt if it references a missing cell.
- The record is corrupt if any referenced cell has a different `commit_id`.
- Duplicate explicit commit records are corrupt.

For legacy raw `StateCell` lines:

- The index should keep reconstructing manifests from cell commit IDs as it does today.
- Mixed logs are allowed so old records and new records can coexist during migration.

## Tests

Add failing tests before implementation:

- New `FileKernel` writes include explicit `cell` and `commit` JSONL records.
- Reopen uses an explicit commit record to preserve manifest lookup and listing.
- Legacy raw `StateCell` logs still open and reconstruct manifests.
- Corrupt explicit commit records referencing missing cells are rejected.

Run the full verification gate before committing implementation:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

## Out of Scope

- File format headers.
- Checksums.
- Fsync or crash recovery.
- Compaction.
- Storage-kernel API changes.
