# Commit Slice Materialization Design

## Goal

Add a native API operation that materializes an ordered range of commit boundaries with the StateCells written by each commit.

## Context

ContinuityDB can list commit manifests with cursor and limit constraints, and it can materialize the StateCells for one commit in manifest order. Audit, backup, sync, and replay callers now need the composed operation: "give me the next commit boundaries and the cells for each boundary."

This belongs in `continuitydb-api` first. Storage kernels already expose the primitive operations: ordered manifest listing and cell lookup. The API layer should compose those primitives into an embeddable operation without expanding the kernel contract.

## Design

Add to `continuitydb-api`:

- `CommitSlice { manifest: CommitManifest, cells: Vec<StateCell> }`
- `ContinuityDb::commit_slices(lookup: CommitManifestLookup) -> Result<Vec<CommitSlice>, ContinuityError>`

The API should:

1. List matching commit manifests using `commit_manifests_matching(lookup)`.
2. For each manifest, hydrate `manifest.cell_ids` using the existing private `lookup_cells_in_order`.
3. Return slices in the exact order returned by the manifest listing.
4. Preserve each manifest in the returned slice.
5. Preserve cell order inside each slice exactly as recorded by that manifest.

## Error Semantics

Unknown cursor commits must continue to return `ContinuityError::Kernel(KernelError::CommitNotFound)` through the existing manifest listing path.

If a manifest references a missing cell, the operation should return the existing `ContinuityError::CellNotFound`. This keeps corrupt or inconsistent backend state distinct from a cursor that does not exist.

## Roadmap Updates

Add a native API milestone for cursor-based commit slice materialization. Update README current scope after implementation.

## Tests

Add failing tests before implementation:

- `commit_slices` returns multiple slices in cursor and limit order.
- Each returned slice preserves its manifest and cells in manifest order.
- `commit_slices` returns `CommitNotFound` for an unknown cursor.
- Empty databases return an empty slice list.

Run the full verification gate before committing implementation:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

## Out of Scope

- Streaming commit slices.
- Storage-kernel batch cell lookup.
- Durable explicit commit records.
- Sync protocols or replication.
- Checksums and repair of inconsistent manifests.
