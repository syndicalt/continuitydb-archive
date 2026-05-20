# Workload Residual Lookup-Plan Regression Design

## Problem

Workload snapshots now preserve file-kernel residual exact lookup constraints. Baseline comparison still ignores that field, so a planner change can add or remove residual exact filtering without producing a workload regression.

For production storage-engine work, residual exact filtering is operationally important. It identifies exact predicates that still need post-index filtering after candidate selection. Changes in that list should be explicit in workload baseline comparisons.

## Goal

Add workload baseline regression detection for residual exact lookup-plan constraints.

## Design

Extend `WorkloadBaselineRegression` with:

```rust
LookupPlanResidualExactConstraintsChanged {
    previous: Vec<String>,
    current: Vec<String>,
}
```

In `push_lookup_plan_regressions`, compare:

```rust
previous.residual_exact_constraints.as_slice()
current.residual_exact_constraints.as_slice()
```

and push the new regression variant when they differ.

## Non-Goals

- Do not change lookup-plan generation.
- Do not change workload snapshot serialization.
- Do not add CLI-specific formatting; serde JSON output for the enum is enough for existing CLI reports.

## Tests

Add a workload regression test where the baseline lookup plan has no residual exact constraints and the current lookup plan adds `valid_at`. The comparison should report `LookupPlanResidualExactConstraintsChanged`.
