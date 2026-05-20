# CLI Local Model Fail-On-Unstable Gate Design

## Goal

Turn local model stability reporting into an optional CI gate. Operators should be able to reject a Steward model candidate when repeated low-temperature benchmark trials produce different decoded proposal outputs.

## Scope

Add `--fail-on-unstable` to `benchmark-local-model`. The flag is only meaningful with `--stability-trials <N>` and must be rejected when used alone. The existing stability report remains available without failing the command.

## Behavior

When `--fail-on-unstable` is present with `--stability-trials <N>` on a real benchmark run:

- Build the normal candidate, runtime config, evaluation suite, and benchmark.
- Run the stability report before recording a new baseline.
- If the report is unstable, return a non-zero CLI error and do not append a baseline record.
- If the report is stable, continue through the existing baseline record and comparison flow.

When used with `--dry-run`, the CLI must not execute the model or create a baseline. The `stability_preflight` object includes:

- `trials`
- `will_execute: false`
- `fail_on_unstable`

When `--fail-on-unstable` is used without `--stability-trials`, the CLI returns an operator-readable error before touching the baseline path.

## Architecture

The change stays inside the existing local-model CLI command:

- Add `fail_on_unstable: bool` to `LocalModelBenchmarkOptions`.
- Add a Clap flag to `Command::BenchmarkLocalModel`.
- Validate that `--fail-on-unstable` requires `--stability-trials`.
- Extend dry-run stability preflight JSON with the gate setting.
- Compute the stability report before baseline recording and return an error before mutation when `fail_on_unstable` is true and the report is not stable.

No new persistence format is added. No library API change is required because `LocalModelStabilityReport::stable()` already exposes the needed gate condition.

## Testing

Add feature-gated CLI tests proving:

- `--fail-on-unstable` without `--stability-trials` fails and does not create a baseline.
- Dry-run with both flags reports `fail_on_unstable: true` and does not create a baseline.
- A real unstable repeated-run benchmark with both flags exits non-zero and does not create a baseline.

Verification must include the focused local-model CLI tests, formatting, clippy, all-features tests, default tests, and `git diff --check`.
