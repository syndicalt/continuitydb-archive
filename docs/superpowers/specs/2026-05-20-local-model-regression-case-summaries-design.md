# Local Model Regression Case Summaries Design

## Problem

Regression reports expose pass-count and failure-code changes, but operators still need to walk embedded per-case reports to know which fixed evaluation cases changed outcome. CI should be able to classify the exact Steward contract cases that regressed or recovered directly from the regression summary.

## Design

- Extend `LocalModelBenchmarkRegression` with ordered case-name lists for:
  - `regressed_case_names`: cases that passed in the previous compatible baseline and fail in the current run.
  - `recovered_case_names`: cases that failed in the previous compatible baseline and pass in the current run.
- Compute case outcome changes by case name using a deterministic ordered union.
- Treat missing case names as failing for defensive compatibility, while compatible suite fingerprints should normally keep the case set stable.
- Surface the lists in CLI `baseline_comparison.regressed_cases` and `baseline_comparison.recovered_cases`.
- Leave existing pass-count and failure-code regression semantics unchanged.

## Test

- Extend the Steward regression test to assert a pass-to-fail case is listed as regressed and no cases are recovered.
- Extend the CLI regression failure-report test to assert the JSON includes the expected regressed case list and an empty recovered case list.
