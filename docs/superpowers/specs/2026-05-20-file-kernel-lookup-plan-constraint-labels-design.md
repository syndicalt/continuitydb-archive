# File Kernel Lookup Plan Constraint Labels Design

## Problem

`FileKernelLookupPlan` reports indexed constraint counts, candidate counts, and full-scan fallback. That is enough to detect whether a lookup uses indexes, but not enough to explain which constraints shaped the candidate set. Operators and embedders need stable planner diagnostics for regression gates and query tuning.

## Design

- Extend `FileKernelLookupPlan` with ordered `indexed_constraints` labels.
- Produce labels from the same internal path that builds indexed candidate sets so labels and counts cannot diverge.
- Use stable snake-case labels matching lookup fields, such as `scope`, `answerability_question`, `dependency_target_kind`, `valid_at`, and `system_at`.
- Preserve the existing `indexed_constraint_count`, `candidate_count`, and `full_scan` fields.
- Expose labels through native API return values and CLI lookup-plan JSON.

This remains an inspection surface only. It does not change lookup ordering, exact filtering, or checkout materialization behavior.

## Test

- Assert unconstrained lookup plans report no indexed labels.
- Assert scope plus answerability lookups report `["scope", "answerability_question"]`.
- Assert native query lookup-plan helpers preserve labels.
- Assert CLI `inspect-kernel --lookup-query` serializes labels in `lookup_plan.indexed_constraints`.
