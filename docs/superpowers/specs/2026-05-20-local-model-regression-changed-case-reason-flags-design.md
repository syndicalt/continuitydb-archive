# Local Model Regression Changed-Case Reason Flags Design

## Problem

Changed-case summaries can now represent pass-state changes, failure-code count changes, and raw response drift. Consumers can infer which condition caused a summary, but that duplicates comparison rules outside the Steward regression API and makes CI logic more brittle.

## Design

- Add explicit boolean reason flags to `LocalModelBenchmarkCaseSummary`:
  - `outcome_changed`
  - `failure_counts_changed`
  - `response_changed`
- Compute these flags once while building changed-case summaries.
- Keep existing regressed and recovered case lists tied only to outcome changes.
- Default the serialized fields for compatibility with older stored summary JSON.
- Expose the flags through existing CLI `baseline_comparison.changed_case_summaries` JSON.

## Test

- Extend the Steward passing-response-change regression test to assert response drift sets only `response_changed`.
- Extend the CLI passing-response-change test to assert the public JSON exposes the same reason flags.
