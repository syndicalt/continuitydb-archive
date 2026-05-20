# Commit Manifest Cursor Listing Design

## Goal

Add cursor-style commit manifest listing so audit, backup, and future sync callers can consume the commit timeline incrementally instead of always listing every manifest.

## Context

ContinuityDB now exposes ordered commit manifest listing. That is enough for inspection, but long-lived embedded databases need bounded reads. A caller should be able to ask for commits after a known commit boundary and optionally cap the number returned.

## Design

Introduce `CommitManifestLookup` in `continuitydb-kernel`:

- `after: Option<CommitId>`: exclusive cursor. When present, listing starts after this visible commit.
- `limit: Option<usize>`: maximum number of manifests to return. `None` means no explicit limit. `Some(0)` returns an empty list.

Extend `StorageKernel` with:

- `list_commit_manifests_matching(lookup: CommitManifestLookup) -> Result<Vec<CommitManifest>, KernelError>`

Keep `list_commit_manifests()` as the convenience method for all manifests by delegating to the default lookup.

## Cursor Semantics

If `after` is present and the commit is not visible, return a kernel error instead of silently returning all or none. This makes invalid replication cursors explicit.

Add:

- `KernelError::CommitNotFound`

The error does not need to carry the commit ID yet because `KernelError` is currently a small comparable enum. A richer diagnostic error can be added when the kernel error model is expanded.

## Kernel Behavior

Memory and file kernels both derive results from their manifest order:

1. Locate the `after` commit if provided.
2. Start at the following index, or index 0 when no cursor is provided.
3. Apply `limit` if present.
4. Return cloned `CommitManifest` values in visibility order.

The file kernel should preserve identical behavior after reopen because its manifest order is reconstructed from the JSONL cell log.

## API

Expose:

- `ContinuityDb::commit_manifests_matching(lookup: CommitManifestLookup) -> Result<Vec<CommitManifest>, ContinuityError>`

This method delegates directly to the kernel. Existing `commit_manifests()` remains the simple all-manifests convenience method.

## Roadmap Updates

Add a storage milestone for cursor-based commit manifest listing. Update README current scope after implementation.

## Tests

Add failing tests before implementation:

- Memory kernel lists manifests after an exclusive cursor.
- Memory kernel applies `limit`.
- Memory kernel reports `CommitNotFound` for an unknown cursor.
- File kernel preserves cursor listing after reopen.
- API exposes cursor listing and preserves kernel order/limit behavior.

Run the full verification gate before committing implementation:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

## Out of Scope

- Reverse scans.
- Time-range scans.
- Pagination tokens separate from `CommitId`.
- Durable replication cursors.
- Commit DAG traversal.
