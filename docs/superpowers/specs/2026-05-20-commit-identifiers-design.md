# Commit Identifiers Design

## Goal

Add a first-class commit identifier to `StateCell` so cells written together can be audited and queried as one database commit boundary. Atomic write batches already share a system transaction time; this slice gives that boundary a stable ID.

## Scope

This slice adds commit identity metadata and lookup filtering. It does not add named transactions, commit records, rollback logs, concurrency control, or crash recovery repair.

## Architecture

`continuitydb-core` adds `CommitId`, a UUID-backed identifier. `StateCell` gains a `commit_id` field with a default nil value for uncommitted or legacy serialized cells.

`StorageKernel` append operations stamp cells with a commit ID. Single-cell append gets a fresh commit ID. Batch append gets one fresh commit ID shared by every cell in the batch. Deterministic variants accept an explicit commit ID:

- `append_cell_at_with_commit_id`
- `append_cells_at_with_commit_id`

Existing `append_cell_at` and `append_cells_at` remain available and generate a fresh commit ID internally.

`CellLookup` gains `commit_id: Option<CommitId>`. Memory and file kernels filter by commit ID. File kernel adds an in-process commit index rebuilt from the JSONL log on open.

## Semantics

Every stored cell has a commit ID after append. Cells in the same accepted batch share both `system_time` and `commit_id`.

Duplicate rejection remains all-or-nothing. A rejected batch does not expose any cells with the requested commit ID.

Legacy serialized cells that lack `commit_id` deserialize with the nil commit ID. This keeps old logs readable while distinguishing legacy/uncommitted data from newly appended commits.

## Testing

Tests must prove:

- New `StateCell` instances start with the nil commit ID.
- Legacy JSON without `commit_id` deserializes to the nil commit ID.
- Memory batch append with explicit commit ID stamps all batch cells and supports commit lookup.
- File batch append with explicit commit ID persists commit IDs across reopen and supports commit lookup.
- API deterministic batch ingest can accept an explicit commit ID and stores cells under that commit.

## Roadmap Impact

This adds the next storage kernel milestone after atomic write batches: explicit commit identity. It prepares future commit records, audit explanations, crash recovery metadata, and query language support for transaction-scoped continuity slices.
