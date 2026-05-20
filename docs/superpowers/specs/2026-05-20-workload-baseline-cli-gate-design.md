# Workload Baseline CLI Gate Design

## Purpose

The workload library can compare a current measurement snapshot against the latest matching baseline. The next frontier step is to expose that comparison through `continuitydb measure-workload` so operator and CI workflows can validate storage-engine changes without writing custom Rust code.

## CLI Behavior

Extend `measure-workload` with:

- `--compare-baseline`: load the latest baseline record matching the current `--label` and `--kernel`.
- `--max-elapsed-growth-percent <N>`: timing tolerance for comparison, defaulting to `25`.
- `--fail-on-regression`: exit non-zero when comparison finds regressions.

Comparison requires `--baseline-path`; without it, the command should fail with a clear error. If no matching baseline exists, the command should succeed and report `baseline_comparison: null`.

When both comparison and recording are requested, comparison must use the latest baseline already present before appending the current measurement. This prevents a run from comparing against itself.

## JSON Output

`measure-workload` should continue printing the measurement JSON. When comparison is requested and a prior baseline exists, add:

- `baseline_comparison.passed`
- `baseline_comparison.baseline_recorded_at`
- `baseline_comparison.regressions`

Regression JSON can use the serde shape of `WorkloadBaselineRegression`; this is an internal operator-facing report for now.

## Tests

Tests must prove:

- CLI comparison succeeds and reports `passed: true` when counts match and elapsed tolerance is large.
- CLI comparison with `--fail-on-regression` exits non-zero when the latest matching baseline has deterministic count differences.
- Comparison requires a baseline path.

## Roadmap Placement

Add Benchmark and Workload milestone 7: CLI workload baseline regression gate.
