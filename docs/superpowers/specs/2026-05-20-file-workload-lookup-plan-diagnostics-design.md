# File Workload Lookup Plan Diagnostics Design

## Problem

File-kernel lookup plans are inspectable for individual queries, but workload measurement artifacts only report ingest and checkout counts/timings. That means benchmark baselines cannot explain whether a workload run used indexes, which constraints were active, or how broad each index candidate set was.

## Design

- Add a shared checkout request to `CellLookup` conversion in `continuitydb-checkout`.
- Use that conversion in checkout materialization so lookup planning and checkout execution share the same storage boundary.
- For `continuitydb measure-workload --kernel file`, report the file kernel lookup plan for the exact deterministic checkout request used by the measurement.
- Keep memory-kernel workload output without a file lookup plan because memory does not expose file-kernel planner diagnostics.
- Reuse the existing CLI lookup-plan JSON shape, including indexed constraint labels and per-constraint cardinalities.

This does not change workload generation, checkout selection, baseline comparison, or storage behavior. It adds planner evidence to file-backed workload artifacts.

## Test

- Add checkout-unit coverage proving `CheckoutRequest` converts into `CellLookup` without token-budget leakage.
- Extend the file-kernel workload CLI test to assert `lookup_plan.indexed_constraints`, `candidate_count`, and `full_scan`.
