# File Kernel Format Header Design

## Goal

Add an explicit format header record to new JSONL `FileKernel` stores so future storage migrations have a durable version anchor.

## Context

The file kernel now writes typed JSONL records for cells and commit manifests, while still accepting legacy raw `StateCell` lines. The format is still missing an explicit version marker. Without one, future changes such as checksums, snapshots, compaction records, or indexed sidecar metadata would need to infer compatibility from record shapes.

## Design

Add an internal header record:

```json
{"type":"header","format":"continuitydb.file_kernel","version":1}
```

New empty files opened by `FileKernel::open` should receive this header before any cells or commits are written. Existing non-empty files without a header remain valid legacy logs. The storage-kernel trait does not change.

## Read Semantics

`FileKernel::open` should:

- Accept a supported header as the first non-empty line.
- Reject unsupported header versions as `KernelError::StoreCorrupt`.
- Reject duplicate headers.
- Reject headers that appear after cell or commit records.
- Continue accepting legacy raw-cell logs that do not contain a header.

## Write Semantics

For a newly created or empty file:

1. `FileKernel::open` writes exactly one header record.
2. Later appends write cell and commit records after the header.

For an existing non-empty legacy file:

1. `FileKernel::open` does not rewrite the file.
2. Future appends continue after existing content.

This avoids mutating legacy files at open time and keeps the append-only story simple.

## Tests

Add failing tests before implementation:

- Opening a new empty `FileKernel` writes the supported header record.
- Appending to a new file preserves header, then cell records, then commit record.
- Legacy raw `StateCell` files still open without a header.
- Unsupported header versions are rejected.
- Headers after data records are rejected.

Run the full verification gate before committing implementation:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

## Out of Scope

- Migrating legacy files to add a header.
- Checksums.
- Compaction.
- File locking.
- Fsync or crash recovery.
