# Commit Manifest Listing Design

## Goal

Add first-class commit manifest listing so ContinuityDB can expose the database commit timeline, not only lookup a manifest when the caller already knows its `CommitId`.

## Context

Commit manifests now record the commit boundary for successful non-empty writes. That supports point lookup, but audit, sync, snapshots, and future replication need ordered discovery of commit boundaries. A datastore should be able to answer "what commits exist?" deterministically.

## Design

Extend the storage kernel with:

- `list_commit_manifests() -> Result<Vec<CommitManifest>, KernelError>`

Ordering is part of the contract:

1. Return manifests in commit visibility order.
2. For the current memory and JSONL file kernels, visibility order is first successful append order.
3. Reopening a file kernel reconstructs the same order from the append-only cell log.

The method returns full `CommitManifest` values rather than IDs only. That keeps the API useful for audit and avoids forcing callers into an N+1 lookup pattern.

## Kernel Behavior

`MemoryKernel` stores manifest order as commits are accepted. Empty batches remain no-ops and do not appear in the list.

`FileKernel` reconstructs manifest order from the JSONL cell log. The first cell seen for a commit establishes that commit's order; later cells with the same commit append to the same manifest's `cell_ids`.

Duplicate commit rejection remains unchanged: a non-empty write using an already visible commit ID is rejected atomically.

## API

Expose:

- `ContinuityDb::commit_manifests() -> Result<Vec<CommitManifest>, ContinuityError>`

The API delegates to the backing kernel. It does not sort or synthesize manifests because the kernel owns commit visibility.

## Roadmap Updates

Add a storage milestone for commit timeline listing. Update README current scope after implementation.

## Tests

Add failing tests before implementation:

- Memory kernel lists manifests in successful append order.
- Memory kernel omits empty batch no-ops from the manifest list.
- File kernel reconstructs manifest listing order after reopen.
- API returns commit manifests in kernel order.

Run the full verification gate before committing implementation:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

## Out of Scope

- Pagination.
- Filtering by time range.
- Reverse ordering.
- Explicit on-disk manifest records.
- Commit DAG traversal.
- Replication cursors.
