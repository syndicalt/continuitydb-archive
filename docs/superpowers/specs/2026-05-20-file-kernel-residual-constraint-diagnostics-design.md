# File Kernel Residual Constraint Diagnostics Design

## Problem

`FileKernelLookupPlan` exposes indexed constraints, exact constraints, lossy indexed constraints, candidate counts, and selectivity. This explains a lot of planner behavior, but it still does not directly identify exact predicates that require residual filtering after candidate retrieval.

For production storage-engine work, operators need to distinguish:

- exact predicates that seed candidate selection through an index,
- indexed predicates that can over-select and need exact filtering,
- exact predicates that still require residual filtering.

The third group is currently implicit: callers have to combine `exact_constraints` and `lossy_indexed_constraints` themselves.

## Goal

Expose ordered residual exact constraint diagnostics from file-kernel lookup plans.

## Terminology

Residual exact constraints are exact lookup predicates present in a `CellLookup` that still require exact filtering after indexed candidate selection. This includes unindexed exact predicates and lossy indexed predicates such as `system_at` and `valid_at`.

## Design

Extend `FileKernelLookupPlan` with:

- `residual_exact_constraint_count: usize`
- `residual_exact_constraints: Vec<&'static str>`

Compute them in `FileKernelIndex::lookup_plan` by preserving exact constraint order and selecting exact constraint names that are absent from `indexed_constraints` or present in `lossy_indexed_constraints`.

Surface the fields through:

- CLI `file_lookup_plan_json`.
- `WorkloadLookupPlanSnapshot` serialization.

This keeps the planner contract deterministic and makes future indexed-kernel work measurable without changing lookup behavior.

## Non-Goals

- Do not add new indexes in this slice.
- Do not change candidate selection or exact filtering behavior.
- Do not change query parsing.
- Do not add workload regression gates for residual constraints yet.

## Tests

Add focused tests:

- Kernel lookup plan reports `valid_at` as a residual exact constraint when the lookup also includes indexed `scope`.
- CLI `inspect-kernel --lookup-query` emits `residual_exact_constraint_count` and `residual_exact_constraints`.
- Workload snapshots preserve the new fields in serialized JSON.
