# Local Model Regression Changed-Case Reason Counts Design

## Problem

Changed-case reason flags make each case self-explanatory, but CI dashboards and lightweight gates still need aggregate counts without walking the full `changed_case_summaries` array.

## Design

- Add top-level aggregate counts to `LocalModelBenchmarkRegression`:
  - `outcome_changed_cases`
  - `failure_count_changed_cases`
  - `response_changed_cases`
- Derive counts from the already-built changed-case summaries to keep aggregate and per-case semantics aligned.
- Preserve existing regression semantics: these counts are diagnostic, while pass-count drops still define baseline regression.
- Expose the counts through CLI `baseline_comparison` JSON.

## Test

- Extend the Steward response-drift regression test to assert a one-case response-only change reports aggregate counts of 0 outcome, 0 failure-count, and 1 response change.
- Extend CLI comparison tests to assert aggregate counts for full-suite same-outcome failure changes and passing response drift.
