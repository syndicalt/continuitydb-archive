# CLI Local Model Stability Reporting Design

## Goal

Expose repeated-run local Steward benchmark stability through the existing `benchmark-local-model` command so operators can detect low-temperature output drift before trusting a local model candidate.

## Scope

Add an optional `--stability-trials <N>` flag to `benchmark-local-model`. The flag is explicit because repeated local model execution can be expensive. Normal benchmark recording and dry-run behavior must remain unchanged when the flag is omitted.

## Behavior

When `--stability-trials <N>` is present on a real benchmark run, the CLI runs the normal benchmark baseline path and also runs `LocalModelBenchmark::run_stability` with the same candidate, runtime config, and evaluation suite. The JSON output includes a top-level `stability` object with:

- `trials`
- `stable`
- `case_reports`

Each case report exposes the public stability report fields already returned by `continuitydb-steward`: case name, stable flag, proposal fingerprints, and changed trial numbers.

When `--stability-trials <N>` is present with `--dry-run`, the CLI must not execute the model or mutate baselines. The dry-run JSON includes a top-level `stability_preflight` object with the configured trial count and `will_execute: false`.

The flag rejects `0` because a requested stability run with zero trials is operator error. The library API may normalize zero for direct embedders, but the CLI should force an explicit positive trial count.

## Architecture

The implementation stays inside the existing local-model CLI path:

- Extend `LocalModelBenchmarkOptions` with `stability_trials: Option<usize>`.
- Add a Clap field to `Command::BenchmarkLocalModel`.
- Validate non-zero trials before dry-run or execution.
- Reuse the already-built `LocalModelBenchmark` for baseline recording and stability execution.
- Serialize the report with `serde_json::to_value` rather than duplicating field mapping.

No new command is added. No stability baseline persistence is added in this slice.

## Testing

Use existing feature-gated CLI tests. Add tests that prove:

- Dry-run reports `stability_preflight` and does not create a baseline file.
- Real benchmark runs include a stable `stability` report and still record one baseline.
- `--stability-trials 0` fails without creating a baseline file.

Verification must include focused CLI tests with `--features local-model`, full formatting, clippy, all-features tests, default tests, and `git diff --check`.
