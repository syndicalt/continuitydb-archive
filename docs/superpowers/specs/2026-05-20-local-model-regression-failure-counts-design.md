# Local Model Regression Failure Counts Design

## Problem

Local Steward benchmark reports now expose aggregate failure counts for the current evaluation, but compatible baseline regression comparisons still only expose pass-count deltas. CI and operators need to know which stable failure classes changed between the previous compatible baseline and the current run without walking both embedded evaluation reports.

## Design

- Extend `LocalModelBenchmarkRegression` with deterministic previous and current failure-count maps keyed by stable failure code.
- Derive the maps from `StewardEvaluationReport::failure_counts` during regression comparison.
- Keep pass-count regression semantics unchanged.
- Surface the maps in CLI `baseline_comparison` JSON for regression reports and normal compared benchmark output.
- Preserve deterministic ordering by using ordered maps.

## Test

- Add a Steward regression test proving previous passing baselines expose empty counts and current failing baselines expose the expected failure code count.
- Add a CLI regression failure-report test proving `baseline_comparison.previous_failure_counts` and `baseline_comparison.current_failure_counts` are present in JSON.
