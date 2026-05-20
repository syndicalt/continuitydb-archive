# File Store Status Design

## Purpose

Kernel capability inspection answers what guarantees a file-backed store provides. Operators also need basic store facts: how many cells are visible, how many commits are visible, and how large the durable file is. This slice adds a native file-store status surface and includes it in CLI kernel inspection output.

## Problem

`inspect-kernel` currently reports capabilities and requirement satisfaction, but it does not report the observed shape of the store. A CI job or operator can verify that a store is durable append-log backed, but cannot quickly tell whether the file is empty, how many commit boundaries it contains, or whether a backup/import operation produced the expected visible count.

## Design

Add `FileKernelStatus` in `continuitydb-kernel`:

- `cell_count`
- `commit_count`
- `file_size_bytes`

Add `FileKernel::status() -> Result<FileKernelStatus, KernelError>`.

The counts come from the rebuilt in-process index, which is already the authoritative visible state after opening and validation. File size comes from filesystem metadata and is fallible.

Expose this through `ContinuityDb<FileKernel>::file_store_status()`.

Extend `continuitydb inspect-kernel` output with:

```json
"status": {
  "cell_count": 0,
  "commit_count": 0,
  "file_size_bytes": 123
}
```

## Non-Goals

- Do not add deep integrity scanning beyond the existing open-time validation.
- Do not expose private index internals.
- Do not add status to generic `StorageKernel`; this status is file-kernel specific because it includes file size.
- Do not change backup, import, or compaction semantics.

## Error Handling

`FileKernel::status` returns `KernelError::StoreIo` if filesystem metadata cannot be read. API and CLI callers propagate that through existing error paths.

## Testing

Tests must prove:

- a new file-backed store reports zero cells, zero commits, and a nonzero file size;
- a committed file-backed store reports the expected cell and commit counts;
- the native API exposes the same status;
- CLI `inspect-kernel` includes the status object.

## Roadmap Impact

This adds the first operational status surface for the durable file kernel. It gives embedders and CI a stable way to inspect visible store shape while the project moves toward a production indexed embedded kernel.
