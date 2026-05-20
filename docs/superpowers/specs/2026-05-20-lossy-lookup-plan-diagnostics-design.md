# Lossy Lookup Plan Diagnostics Design

## Context

File-kernel lookup plans currently report indexed constraints, exact constraints, candidate counts, filtered candidate counts, and selectivity. This tells operators how much residual filtering happened, but not which indexed predicates are known to over-select before exact filtering.

All current exact lookup predicates have an index-backed candidate source, so reporting "unindexed exact constraints" would be empty. The useful production signal is whether an indexed predicate is lossy.

## Design

Add ordered `lossy_indexed_constraints` and `lossy_indexed_constraint_count` fields to `FileKernelLookupPlan`.

A lossy indexed constraint is an index that can produce candidates that still fail the exact predicate because the index stores only a coarse boundary. Initially:

- `valid_at` is lossy because the file index selects cells whose valid start is before or equal to the requested time; exact filtering must still check the valid end.
- `system_at` is lossy because the file index selects cells whose system start is before or equal to the requested time; exact filtering must still check the system end.

All other current indexed constraints remain exact candidate sources.

## Surfaces

Expose the new fields through:

- `FileKernelLookupPlan` in `continuitydb-kernel`.
- CLI lookup-plan JSON.
- Workload lookup-plan snapshots and baseline regression comparison.
- README and roadmap milestone text.

## Testing

Use test-first coverage at the kernel boundary, CLI JSON boundary, and workload artifact boundary:

- Kernel test: a lookup containing `valid_at`, `system_at`, and `scope` reports only `valid_at` and `system_at` as lossy.
- CLI test: `inspect-kernel --query` JSON includes the lossy temporal labels.
- Workload test: serialized workload lookup-plan snapshots preserve lossy labels and count.

