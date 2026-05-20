# File Lookup Plan Exact Match Count Design

## Problem

`FileKernelLookupPlan.candidate_count` reports the number of StateCell positions selected
by indexed candidate planning before exact predicate filtering. That is useful for index
selectivity, but it does not show how many candidates will actually satisfy the full
lookup predicate. Operators and benchmark gates need both numbers to distinguish broad
candidate seeding from exact checkout selectivity.

## Decision

Add `exact_match_count` to file lookup plans:

- `candidate_count` remains the pre-filter candidate count.
- `exact_match_count` counts candidate StateCells that satisfy the same predicates used
  by `lookup_cells`.
- CLI lookup-plan JSON includes `exact_match_count`.
- Workload lookup-plan snapshots preserve `exact_match_count` for benchmark baselines
  and regression comparisons.

## Verification

Use a valid-time lookup where the valid-time index selects two cells by start time but
exact half-open range filtering keeps only one. The plan should report:

- `candidate_count = 2`
- `exact_match_count = 1`

Add focused tests at kernel, CLI, and workload-snapshot levels before implementation.
