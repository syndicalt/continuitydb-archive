# File Kernel System-Time Index Design

## Problem

`CellLookup.system_at` is a first-class transaction-time constraint used by checkout and query execution, but the JSONL file kernel still uses the full append-order cell vector as the candidate set for system-time-only queries. That keeps behavior correct, but transaction-time lookup is central to audit and as-of materialization, so the durable reference kernel should maintain a derived access path.

## Design

- Add a derived vector of `(system_time.from(), cell_position)` entries to `FileKernelIndex`.
- Rebuild the vector from canonical or legacy JSONL records during `FileKernel::open`.
- Maintain the vector after successful appends.
- Use the system-time index as the candidate set when `CellLookup.system_at` is the strongest available lookup constraint.
- Preserve the final `SystemTimeRange::contains` predicate so exact range semantics are retained if closed system-time ranges are introduced later.

## Test

- Add a file-kernel test proving system-time indexes are rebuilt after reopen and produce the same result as `lookup_cells`.
- Add a file-kernel test proving system-time indexes are updated after append.
