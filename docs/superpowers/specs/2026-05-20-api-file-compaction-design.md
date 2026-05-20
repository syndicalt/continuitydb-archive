# API File Compaction Design

## Goal

Expose JSONL file-kernel compaction through the native embeddable API for `ContinuityDb<FileKernel>`.

## Context

`FileKernel::compact` can rewrite a durable JSONL log into the current canonical record format. Applications that embed ContinuityDB through `ContinuityDb<K>` should not need to break abstraction by manually calling `kernel_mut().compact()` for routine storage maintenance.

Compaction remains file-kernel-specific. The generic `StorageKernel` trait should not grow until multiple durable backends share a stable maintenance contract.

## Design

Add an inherent impl in `continuitydb-api`:

```rust
impl ContinuityDb<FileKernel> {
    pub fn compact_file_store(&mut self) -> Result<(), ContinuityError>
}
```

The method delegates to `self.kernel.compact()` and maps `KernelError` through `ContinuityError::Kernel`.

## Semantics

The API method should preserve all behavior already guaranteed by `FileKernel::compact`:

- stored cells remain queryable;
- commit manifests remain listed in visibility order;
- cursor manifest listing still works after reopen;
- legacy raw logs are rewritten into canonical header plus checksummed cell and commit records.

## Tests

Add failing API tests before implementation:

- `compact_file_store` rewrites a file-backed database to canonical records.
- `compact_file_store` preserves commit slices after reopen.

Run the full verification gate before committing implementation:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

## Out of Scope

- Adding compaction to the generic `StorageKernel` trait.
- Memory-kernel compaction.
- CLI compaction command.
- Compaction policies or automatic background maintenance.
