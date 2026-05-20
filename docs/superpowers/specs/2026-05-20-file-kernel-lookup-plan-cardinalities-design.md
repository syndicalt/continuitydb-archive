# File Kernel Lookup Plan Cardinalities Design

## Problem

Lookup plans now expose indexed constraint labels and the final candidate count. That explains which indexes are in play, but not how selective each index is before intersection. Without per-constraint cardinalities, embedders cannot distinguish one narrow index plus one broad index from two equally selective indexes.

## Design

- Add `FileKernelIndexedConstraintPlan` with a stable constraint `name` and single-index `candidate_count`.
- Add ordered `indexed_constraint_plans` to `FileKernelLookupPlan`.
- Build per-constraint plans from the same indexed candidate constraints used for labels and intersection.
- Preserve existing top-level `indexed_constraint_count`, `indexed_constraints`, `candidate_count`, and `full_scan`.
- Serialize per-constraint plans through CLI lookup-plan JSON.

This is diagnostic-only. It does not change lookup, filtering, checkout, or materialization behavior.

## Test

- Assert a scope plus answerability lookup reports per-index counts before intersection.
- Assert native API lookup-plan values expose the same per-constraint counts.
- Assert CLI `inspect-kernel --lookup-query` serializes `indexed_constraint_plans` with names and candidate counts.
