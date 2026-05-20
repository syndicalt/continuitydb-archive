# File Kernel Lookup Plan Design

## Problem

The file kernel now has deterministic candidate selection and intersection, but embedders cannot inspect whether a lookup will start from derived indexes or fall back to all visible cells. That makes it harder to build workload gates, operator diagnostics, and future planner regressions.

## Design

- Add `FileKernelLookupPlan`.
- Expose `FileKernel::lookup_plan(&CellLookup)`.
- Report the number of indexed constraints present in the lookup.
- Report the number of candidate StateCells selected before final exact predicate filtering.
- Report whether the lookup is a full scan.
- Keep the API file-kernel-specific so the generic `StorageKernel` trait does not commit every backend to identical planner metadata.

This is an observability API, not a new execution path. It reuses the same candidate-planning helpers as `lookup_cells`.

## Test

- A default lookup with no indexed constraints reports `full_scan = true` and all visible cells as candidates.
- A constrained lookup with scope and answerability indexes reports two indexed constraints and the intersected candidate count.
