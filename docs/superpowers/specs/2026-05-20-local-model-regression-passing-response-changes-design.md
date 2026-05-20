# Local Model Regression Passing Response Changes Design

## Problem

Baseline comparisons can currently miss local Steward model drift when both previous and current outputs still pass the fixed evaluation case and failure-code counts are unchanged. That hides prompt/runtime/model changes that alter the produced proposal text while preserving the score.

## Design

- Treat a per-case response fingerprint change as a changed-case summary condition.
- Preserve `regressed_cases` and `recovered_cases` as pass-state transitions only.
- Preserve `regressed` as pass-count or all-pass regression only; response drift is diagnostic, not a failure gate by itself.
- Reuse the previous/current response fingerprint and byte-count fields already present on `LocalModelBenchmarkCaseSummary`.
- Surface passing response changes through existing CLI `baseline_comparison.changed_case_summaries` JSON.

## Test

- Add a Steward regression test where previous and current baselines both pass the same case with different rationales and distinct raw response fingerprints.
- Add a CLI benchmark comparison test where two passing full-suite runs differ only in raw model response text and assert the JSON contains a same-passing changed-case summary with distinct response fingerprints.
