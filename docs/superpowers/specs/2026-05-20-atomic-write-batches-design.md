# Atomic Write Batches Design

## Goal

Add a kernel-level atomic StateCell batch append operation. ContinuityDB should be able to commit multiple immutable `StateCell` versions as one deterministic write boundary, with one shared system transaction time and all-or-nothing visibility.

## Scope

This slice adds the first write-batch contract. It does not add named transactions, rollback logs, concurrent writers, crash recovery records, compaction, or multi-file storage. Those belong to later storage-engine milestones.

The batch operation is still append-only. It does not mutate existing cells, retire predecessors, or apply conflict-resolution decisions.

## Architecture

`StorageKernel` gains:

- `append_cells(cells: impl IntoIterator<Item = StateCell>)`
- `append_cells_at(cells: impl IntoIterator<Item = StateCell>, committed_at: DateTime<Utc>)`

`append_cells` stamps the batch with `Utc::now()`. `append_cells_at` is the deterministic primitive used by tests and higher layers.

`MemoryKernel` validates the whole batch before pushing any cell. `FileKernel` validates the whole batch against its in-memory index and against duplicate IDs inside the batch before serializing or writing. After validation, the file kernel stamps every cell with the same `SystemTimeRange::open_from(committed_at)`, serializes the full batch, writes the contiguous JSONL records, and updates indexes only after the write succeeds.

`ContinuityDb<K>` exposes:

- `ingest_cells(cells) -> Result<Vec<StateCellId>, ContinuityError>`
- `ingest_cells_at(cells, committed_at) -> Result<Vec<StateCellId>, ContinuityError>`

Returned IDs preserve caller order.

## Semantics

Empty batches are valid no-ops and return an empty ID list through the API.

Every accepted cell in a batch receives the same system transaction time. This gives future checkout, audit, and revision code a real commit boundary to query with `system_at`.

Duplicate IDs already present in storage reject the whole batch with `KernelError::DuplicateCell`. Duplicate IDs repeated inside the same batch also reject the whole batch with `KernelError::DuplicateCell`.

On duplicate rejection, no batch cell becomes visible through lookup. For `FileKernel`, no new records are written for validation failures. For write I/O failures, this slice guarantees that the in-memory index is not advanced unless the write call succeeds; full crash recovery and partial-write repair are later storage-engine work.

## Testing

Tests must prove:

- Memory batches append all cells with the same system time.
- Memory batches reject duplicate IDs inside the batch without appending any batch cell.
- File batches persist all cells across reopen with the same system time.
- File batches reject duplicate IDs inside the batch without writing records.
- File batches reject IDs already stored without writing records.
- API batch ingest returns IDs in input order.
- API batch ingest reports duplicate failures without partial visibility.

## Roadmap Impact

This adds the next storage-kernel milestone: atomic batch append as the first transaction-boundary primitive. It prepares the path for durable commit records, crash recovery, richer transaction APIs, and Steward/policy flows that need to commit related evidence and audit cells together.
