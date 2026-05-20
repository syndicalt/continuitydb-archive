# File Lookup Plan Selectivity Design

## Context

File lookup plans expose candidate counts, exact match counts, filtered candidate counts, indexed constraints, and exact predicates. Operators can compute selectivity from those counts, but durable workload artifacts and CI gates should not need to rederive that signal differently in each consumer.

## Design

Add a deterministic candidate selectivity score to file lookup plans:

- `candidate_selectivity_basis_points` is an integer basis-point score.
- It is calculated as `exact_match_count * 10_000 / candidate_count`.
- A plan with zero candidates reports `0`.
- `10_000` means every candidate survived exact filtering.
- `0` means no candidate survived or no candidates existed.

This metric is intentionally integer-only so JSON artifacts are stable across platforms and future language bindings.

## Surfaces

- `FileKernelLookupPlan` exposes the field for native embedders.
- CLI lookup-plan JSON serializes the field.
- Workload snapshots preserve the field for benchmark artifacts.
- Workload baseline comparison reports regressions when candidate selectivity changes.

## Test Scenario

Use a `valid_at` lookup where the valid-start index returns two candidates and exact half-open valid-time filtering keeps one:

- `candidate_count = 2`
- `exact_match_count = 1`
- `filtered_candidate_count = 1`
- `candidate_selectivity_basis_points = 5000`

