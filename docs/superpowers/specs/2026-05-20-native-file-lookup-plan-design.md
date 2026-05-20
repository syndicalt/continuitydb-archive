# Native File Lookup Plan Design

## Problem

`FileKernel::lookup_plan` exposes useful planner diagnostics, but embedders should not have to reach through `ContinuityDb<FileKernel>::kernel()` to inspect file-backed lookup behavior. Native file-store status and health already have file-specific API methods; lookup planning should follow that pattern.

## Design

- Import `FileKernelLookupPlan` into `continuitydb-api`.
- Add `ContinuityDb<FileKernel>::file_lookup_plan(&CellLookup)`.
- Delegate directly to the backing `FileKernel`.
- Keep the method file-kernel-specific instead of adding planner metadata to the generic `ContinuityDb<K>` API.

## Test

- Open a file-backed `ContinuityDb`.
- Ingest cells where scope and answerability constraints produce an indexed intersection.
- Assert `file_lookup_plan` reports two indexed constraints, one pre-filter candidate, and no full scan.
