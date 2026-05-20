# File Kernel Valid-Time Index Design

## Problem

`CellLookup.valid_at` is a first-class real-world time constraint used by checkout and text query execution, but the JSONL file kernel still uses the full append-order cell vector as the candidate set for valid-time-only queries. That keeps behavior correct, but bitemporal lookup is core to ContinuityDB's storage model and should have a derived access path in the durable reference kernel.

## Design

- Add a derived vector of `(valid_time.from(), cell_position)` entries to `FileKernelIndex`.
- Rebuild the vector from canonical or legacy JSONL records during `FileKernel::open`.
- Maintain the vector after successful appends.
- Use the valid-time index as the candidate set when `CellLookup.valid_at` is the strongest available lookup constraint.
- Preserve the final `ValidTimeRange::contains` predicate so exact half-open range semantics are retained.

## Test

- Add a file-kernel test proving valid-time indexes are rebuilt after reopen and produce the same result as `lookup_cells`.
- Add a file-kernel test proving valid-time indexes are updated after append.
