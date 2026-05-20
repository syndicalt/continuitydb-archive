# File Lookup Plan Exact Constraints Design

## Context

File lookup plans now expose indexed constraint labels, per-index candidate counts, final candidate counts, exact match counts, and filtered candidate counts. Operators can see how many candidates were rejected, but the plan does not yet state which exact lookup predicates are applied to prove candidates after index seeding.

## Design

Add exact predicate labels to file lookup plans:

- `exact_constraint_count` is the number of lookup predicates present in the request.
- `exact_constraints` is the ordered stable list of predicates checked by exact filtering.
- Labels use the same public vocabulary as indexed constraints: `cell_id`, `semantic_anchor`, `commit_id`, `scope`, `answerability_question`, `evidence_source`, `activation`, `dependency_target`, `dependency_kind`, `dependency_target_kind`, `minimum_confidence`, `system_at`, and `valid_at`.
- Dependency target plus dependency kind remains one combined predicate, `dependency_target_kind`, because exact matching evaluates both on the same dependency edge.
- Default empty lookups report zero exact constraints.

## Surfaces

- `FileKernelLookupPlan` exposes the fields for native embedders.
- CLI lookup-plan JSON serializes the fields.
- Workload snapshots preserve the fields for benchmark artifacts.
- Workload baseline comparison reports regressions when exact predicate labels change.

## Test Scenario

Use a lookup with `valid_at` and `scope`:

- Both predicates appear in `exact_constraints`.
- `exact_constraint_count = 2`.
- Existing indexed and count diagnostics remain unchanged.

