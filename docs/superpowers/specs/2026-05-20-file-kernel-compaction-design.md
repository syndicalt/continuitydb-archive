# File Kernel Compaction Design

## Goal

Add a deterministic file-kernel compaction operation that rewrites the durable JSONL log into the latest canonical record format while preserving all visible StateCells and commit manifests.

## Context

The JSONL `FileKernel` now supports legacy raw StateCell logs, typed cell and commit envelope records, versioned headers, and per-record checksums. That gives the storage layer enough structure to perform a first compaction pass.

Because StateCells are immutable, this initial compaction does not discard history. It rewrites the same stored cells and commit manifests into a clean canonical log:

1. one supported header record;
2. all visible cells in current storage order;
3. commit records in current manifest order;
4. checksums on every cell and commit record.

This is a storage maintenance primitive, not a semantic pruning feature.

## API

Add an inherent method on `FileKernel`:

```rust
pub fn compact(&mut self) -> Result<(), KernelError>
```

The method stays file-kernel-specific. The `StorageKernel` trait should not grow until compaction semantics are proven across more than one durable backend.

## Write Semantics

Compaction should:

- write the canonical log to a temporary file in the same directory;
- flush the temporary file;
- atomically rename the temporary file over the current file with `fs::rename`;
- rebuild the in-memory index from the canonical log after rename succeeds.

If writing or parsing the temporary compacted log fails, the original file and in-memory index should remain unchanged.

## Tests

Add failing tests before implementation:

- Compacting a legacy raw-cell log rewrites it with a header, checksummed cell records, and a checksummed commit record.
- Compaction preserves cell lookup and commit manifest listing.
- Compaction preserves cursor commit manifest listing.
- Compaction of an empty new file leaves a valid single-header log.

Run the full verification gate before committing implementation:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

## Out of Scope

- Semantic history pruning.
- Snapshot records.
- File locking.
- Fsync-level crash guarantees.
- Cross-backend compaction trait methods.
