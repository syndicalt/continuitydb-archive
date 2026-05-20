# Local Model Regression Same-Outcome Failure Changes Design

## Problem

Regression summaries report aggregate failure-code deltas and changed case outcomes, but a local Steward model can keep the same pass count while moving failure reasons inside a still-failing case. CI needs a per-case signal for those quality shifts without treating every flat pass-count comparison as clean.

## Design

- Reuse `LocalModelBenchmarkCaseSummary` for same-outcome changed failure reasons.
- Include a case when previous and current pass state differ or when the per-case stable failure-code delta map is non-empty.
- Keep the existing deterministic ordered case-name union.
- Preserve `regressed_cases` and `recovered_cases` as pass-state changes only, so same-outcome shifts do not trip pass-count regression gates by themselves.
- Surface the unchanged-pass-state summaries through existing CLI `baseline_comparison.changed_case_summaries` JSON.

## Test

- Add a Steward regression test where previous and current baselines both fail and have zero pass-count delta, but the current case adds a different failure code.
- Add a CLI benchmark comparison test that writes a failing baseline, compares another failing run with changed failure reasons, and asserts `changed_case_summaries` includes a same-outcome case with non-empty failure deltas while `regressed` remains false.
