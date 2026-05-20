# Local Model Regression Changed Case Summaries Design

## Problem

Regression reports identify which case names regressed or recovered, but they still require consumers to inspect embedded per-case reports to understand why each changed case moved. CI should be able to inspect a structured changed-case summary that includes previous/current pass state and stable failure-code deltas for that case.

## Design

- Add a serializable `LocalModelBenchmarkCaseSummary` value for changed compatible-baseline cases.
- Include the case name, previous pass state, current pass state, previous failure-code counts, current failure-code counts, and current-minus-previous failure-code deltas.
- Store ordered changed-case summaries on `LocalModelBenchmarkRegression`.
- Derive regressed and recovered case-name lists from the summaries to keep the comparison surfaces consistent.
- Surface the summaries in CLI `baseline_comparison.changed_case_summaries`.

## Test

- Extend the Steward regression test to assert the changed case summary reports the regressed case name, previous/current pass state, and `missing_citation` delta.
- Extend the CLI regression failure-report test to assert the conflict-classification changed-case summary includes previous/current pass state and the expected `missing_expected_action` delta.
