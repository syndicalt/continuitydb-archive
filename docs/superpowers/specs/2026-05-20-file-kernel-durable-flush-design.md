# File Kernel Durable Flush Design

## Goal

Make successful JSONL file-kernel writes cross an explicit filesystem durability boundary before the in-memory index is advanced or a compaction is reported as complete.

## Current Behavior

The file kernel writes append batches to the backing file and then updates the in-memory index. The compaction path writes a temporary file, flushes it, validates it, renames it over the backing file, and updates the index. `flush` only pushes Rust/std buffering to the OS; it does not guarantee the bytes or rename metadata have reached durable storage.

## Proposed Behavior

Add small internal helpers:

```rust
fn write_all_durable(file: &mut File, bytes: &[u8]) -> Result<(), KernelError>
fn sync_parent_directory(path: &Path) -> Result<(), KernelError>
```

Use `write_all_durable` for:

- new-store header writes;
- append batch writes;
- compaction temporary-file writes.

Use `sync_parent_directory` after compaction rename so the replacement metadata is also pushed to the filesystem boundary where supported.

The append order should remain:

1. Validate duplicates and commit ID.
2. Encode all records.
3. Write, flush, and sync the bytes.
4. Only then update the in-memory index.

## Non-Goals

- Do not introduce a new storage format.
- Do not implement full crash-recovery truncation or repair.
- Do not add platform-specific fsync knobs.
- Do not change the `StorageKernel` trait.

## Tests

Add internal tests that prove:

- `write_all_durable` writes bytes to a file and leaves the file reopenable with those bytes.
- `sync_parent_directory` returns `Ok(())` for a normal temporary store path.
- Appended data remains visible after reopen, preserving existing behavior with the new durable helper in the write path.

The first test is intentionally helper-level because actual power-loss durability is not unit-testable in-process. The implementation still moves the production write path to the helper.
