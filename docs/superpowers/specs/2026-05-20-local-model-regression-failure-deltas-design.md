# Local Model Regression Failure Deltas Design

## Problem

Regression reports expose previous and current failure-code counts, but operators still need to subtract the maps manually to know which failure classes were introduced, increased, reduced, or cleared. CI should be able to inspect one deterministic map for the failure-code deltas that caused or accompanied a local Steward model regression.

## Design

- Add a deterministic `failure_count_deltas` map to `LocalModelBenchmarkRegression`.
- Compute each delta as current count minus previous count for the union of stable failure codes.
- Omit zero deltas to keep reports compact and focused on changed failure classes.
- Preserve deterministic ordering with `BTreeMap`.
- Surface the map in CLI `baseline_comparison.failure_count_deltas` JSON.
- Leave existing pass-count regression semantics unchanged.

## Test

- Extend the Steward regression test to assert a newly introduced `missing_citation` failure has delta `1`.
- Extend the CLI regression failure-report test to assert `missing_expected_action` and `missing_citation` deltas are present in JSON.
