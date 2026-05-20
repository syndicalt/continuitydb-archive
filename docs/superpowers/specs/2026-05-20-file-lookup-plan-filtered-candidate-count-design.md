# File Lookup Plan Filtered Candidate Count Design

## Problem

File lookup plans now report both indexed candidate count and exact post-filter match
count. Operators can derive how many candidates were discarded by exact predicates,
but every CLI consumer, workload baseline, or dashboard would need to recompute the
same diagnostic.

## Decision

Add `filtered_candidate_count` to file lookup plans:

- `candidate_count` remains the pre-filter indexed candidate count.
- `exact_match_count` remains the post-filter match count.
- `filtered_candidate_count` is `candidate_count - exact_match_count`.
- CLI JSON and workload baseline snapshots preserve the field.
- Workload baseline comparison reports deterministic regressions when the filtered
  candidate count changes.

## Verification

Use the existing valid-time scenario where two candidates are seeded by the index and
one is rejected by exact half-open range filtering. The plan should report:

- `candidate_count = 2`
- `exact_match_count = 1`
- `filtered_candidate_count = 1`

Add focused kernel, CLI, workload snapshot, and workload regression tests before
implementation.
