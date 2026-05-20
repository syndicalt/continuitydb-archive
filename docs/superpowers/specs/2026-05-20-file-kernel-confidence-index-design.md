# File Kernel Confidence Index Design

## Problem

`CellLookup.minimum_confidence` is a first-class storage and checkout constraint, but the JSONL file kernel still uses the full append-order cell vector as the candidate set for confidence-only queries. That keeps behavior correct, but confidence is central to evidence-backed checkout and should have a derived access path in the durable reference kernel.

## Design

- Add a derived vector of `(max_evidence_confidence, cell_position)` entries to `FileKernelIndex`.
- Rebuild the vector from canonical or legacy JSONL records during `FileKernel::open`.
- Maintain the vector after successful appends.
- Use the confidence index as the candidate set when `CellLookup.minimum_confidence` is the strongest available lookup constraint.
- Preserve the final predicate filter so combined lookups and future precision changes keep exact semantics.

The index stores each cell's maximum evidence confidence because `minimum_confidence` matches a cell when any evidence item meets the threshold.

## Test

- Add a file-kernel test proving confidence indexes are rebuilt after reopen and produce the same result as `lookup_cells`.
- Add a file-kernel test proving confidence indexes are updated after append.
