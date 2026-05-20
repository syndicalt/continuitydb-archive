# File Kernel Scope Index Design

## Problem

`CellLookup.scope` is a first-class storage and checkout constraint, but the JSONL file kernel still uses the full append-order cell vector as the candidate set for scope-only queries. That keeps behavior correct, but it leaves one of the core context boundaries unindexed in the durable reference kernel.

## Design

- Add a derived `Scope -> Vec<cell position>` map to `FileKernelIndex`.
- Rebuild the map from canonical or legacy JSONL records during `FileKernel::open`.
- Maintain the map after successful appends using the same in-memory position used by other derived indexes.
- Use the scope index as the candidate set when `CellLookup.scope` is the strongest available lookup constraint.
- Keep the existing final predicate filters so combined lookups preserve exact semantics.

`Scope` becomes hashable so it can be used as a stable in-process index key.

## Test

- Add a file-kernel test proving scope indexes are rebuilt after reopen and produce the same result as `lookup_cells`.
- Add a file-kernel test proving scope indexes are updated after append.
