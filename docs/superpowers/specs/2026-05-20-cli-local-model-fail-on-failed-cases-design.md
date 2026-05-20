# CLI Local Model Failed-Case Gate Design

## Goal

Make fixed Steward evaluation failures usable as a CI gate before real local model trials become part of the normal workflow.

## Scope

Add `--fail-on-failed-cases` to `benchmark-local-model`. The flag causes the command to exit non-zero when any fixed evaluation case fails, and it must do so before appending a new baseline record.

## Behavior

When the flag is absent, existing behavior remains unchanged: the command records the benchmark baseline and reports `passed`, `passed_cases`, `failed_cases`, `total_cases`, and per-case failures in JSON.

When `--fail-on-failed-cases` is present on a real benchmark run:

- Run the local model benchmark once.
- Build the current benchmark baseline in memory.
- If `evaluation_summary.passed()` is false, return an operator-readable error and do not create or append the baseline file.
- If all cases pass, continue through compatible baseline comparison and baseline recording.

When used with `--dry-run`, the command must not execute the model or mutate baselines. The dry-run JSON includes `fail_on_failed_cases: true` so operators can inspect the configured gate.

This is separate from `--fail-on-regression`: failed-case gating evaluates the current run against the fixed suite, while regression gating compares current results with a previous compatible baseline.

## Architecture

Use the existing `LocalModelBenchmark`, `LocalModelBenchmarkBaseline::from_report`, `latest_compatible_local_model_benchmark_baseline`, and `LocalModelBenchmarkRegression` APIs. Avoid `record_local_model_benchmark_baseline_with_regression` in the CLI path because that helper records before the CLI can reject failed current cases.

The CLI path should:

1. Run optional stability gate first, as it already does.
2. Run the benchmark and create a current baseline in memory.
3. Apply failed-case gate.
4. Compare with latest compatible baseline only if requested.
5. Apply regression gate.
6. Append the baseline.
7. Serialize the existing output shape.

## Testing

Add RED tests proving:

- Dry-run reports `fail_on_failed_cases: true` and creates no baseline.
- A failing fixed evaluation run with the gate exits non-zero and creates no baseline.
- A passing fixed evaluation run with the gate records one baseline.

Verification must include focused CLI tests, formatting, clippy, all-features tests, default tests, and `git diff --check`.
