# Commit Materialization API Design

## Goal

Add a native API operation that materializes the ordered StateCells written by one database commit.

## Context

ContinuityDB now has commit manifests, ordered commit listing, and cursor-based commit listing. A caller can discover a commit boundary and inspect the ordered `StateCellId`s it wrote, but still has to manually hydrate those cells. Audit, backup, sync, and debugging flows need a direct operation for "give me the cells from this commit in manifest order."

## Design

Add to `continuitydb-api`:

- `ContinuityDb::commit_cells(commit_id: CommitId) -> Result<Vec<StateCell>, ContinuityError>`

The API should:

1. Lookup the commit manifest using `commit_manifest(commit_id)`.
2. Return `ContinuityError::Kernel(KernelError::CommitNotFound)` when no manifest exists.
3. Hydrate `manifest.cell_ids` using the existing private `lookup_cells_in_order`.
4. Return cells in the exact order recorded by the manifest.

The method belongs in the native API layer, not the storage kernel. Kernels already provide the primitive operations: manifest lookup and cell lookup. The API layer is the correct place to compose those primitives into an ergonomic embeddable operation.

## Error Semantics

Unknown commit IDs produce `KernelError::CommitNotFound` through `ContinuityError::Kernel`. Missing cells referenced by a manifest still produce the existing `ContinuityError::CellNotFound`. That distinction matters: an unknown commit is different from a corrupt or inconsistent backend manifest.

## Roadmap Updates

Add a native API milestone for ordered commit materialization. Update README current scope after implementation.

## Tests

Add failing tests before implementation:

- `commit_cells` returns cells in manifest order for a multi-cell commit.
- `commit_cells` returns `CommitNotFound` for an unknown commit.

Run the full verification gate before committing implementation:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

## Out of Scope

- Streaming commit cells.
- Range materialization across many commits.
- Checksum verification.
- Repairing inconsistent manifests.
- Storage-kernel batch lookup APIs.
