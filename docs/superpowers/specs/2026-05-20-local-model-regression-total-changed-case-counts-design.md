# Local Model Regression Total Changed-Case Counts Design

## Problem

Regression reports expose reason-specific changed-case counts, but consumers still need a single total changed-case count for dashboards and lightweight gates without scanning `changed_case_summaries`.

## Design

- Add `changed_cases` to `LocalModelBenchmarkRegression`.
- Derive it from `changed_case_summaries.len()` so it matches the emitted records.
- Expose the count through CLI `baseline_comparison.changed_cases`.
- Keep reason-specific counts unchanged for breakdowns.

## Test

- Extend the Steward passing-response-change test to assert a one-case comparison reports `changed_cases == 1`.
- Extend CLI comparison tests to assert full-suite changed-case totals are exposed in JSON.
