# Native Operation API Design

## Goal

Add the first native embeddable operation API for ContinuityDB. The API should let applications use typed operations for ingest, checkout, and audit without stitching together storage kernels and checkout functions directly.

## Scope

This slice adds a small `continuitydb-api` crate and the storage support it needs. It does not add a text query language, parser, network server, async runtime, or agent runtime.

## Architecture

`continuitydb-api` owns a generic `ContinuityDb<K>` wrapper over any `StorageKernel`. The wrapper exposes deterministic database operations:

- `ingest_cell`
- `ingest_cell_at`
- `checkout`
- `audit_cell`

`audit_cell` requires first-class lookup by immutable `StateCellId`, so `CellLookup` gains an optional `cell_id` filter. Memory and file kernels implement the filter consistently. File-backed lookup uses the existing ID index rather than scanning the whole log when a cell ID is supplied.

## Semantics

Ingest remains append-only. The API returns the ingested `StateCellId` after the kernel accepts the cell. Duplicate IDs continue to fail through the kernel error path.

Checkout delegates to the existing deterministic checkout engine. The API does not change ranking, filtering, token budgets, audit traces, alternatives, or uncertainty metadata.

Audit resolves a stored cell by ID and returns the same `AuditTrace` produced by `continuitydb-checkout::audit`. Missing IDs return a typed `CellNotFound` API error.

## Error Handling

`ContinuityError` wraps kernel and checkout errors and adds `CellNotFound`. The API must not panic, unwrap, or hide storage failures.

## Testing

The implementation is test-first:

- Memory and file kernels must prove `CellLookup.cell_id` returns exactly the matching cell.
- `ContinuityDb::ingest_cell_at` must append through the kernel and return the cell ID.
- `ContinuityDb::checkout` must return a deterministic checkout slice from ingested cells.
- `ContinuityDb::audit_cell` must return structured audit evidence for a stored cell and `CellNotFound` for a missing ID.

## Roadmap Impact

This creates the first native API milestone: typed operations over kernel, checkout, and audit. It is the precursor to any future query language because it defines the stable operation boundary that syntax can later compile into.
