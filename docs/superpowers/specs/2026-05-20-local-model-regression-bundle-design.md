# Local Model Regression Bundle Design

## Goal

Make `benchmark-local-model --artifact-dir --fail-on-regression` preserve a complete artifact bundle when a compatible previous baseline detects a pass-count regression.

## Context

Fixed evaluation failures now write artifact bundles before exiting non-zero. Regression failures still return before the command wrapper can write the artifact-directory report and root bundle manifest, which loses the comparison evidence needed to debug CI failures.

## Design

When a real benchmark run detects a regression and `--artifact-dir` is present, the CLI writes the same bundle shape used by successful runs:

- `<artifact-dir>/benchmark-report.json`
- `<artifact-dir>/local-model-benchmark.manifest.json`
- contracts, prompts, responses, and nested response manifest where applicable

The report includes `baseline_comparison.regressed = true`, previous/current pass counts, pass-count delta, and `bundle_manifest` metadata. The command still exits non-zero and does not append the regressed run as a new baseline.

## Boundaries

- This does not change fixed-suite failure behavior.
- This does not change dry-run preflight behavior.
- This does not record regressed runs as baselines.
- `--artifact-dir` is the default bundle location; explicit artifact-kind directory overrides still apply.

## Testing

Add a CLI test that first records a passing compatible baseline, then runs a lower-quality model response with `--artifact-dir --fail-on-regression`. The test verifies non-zero exit, unchanged baseline record count, report and root manifest creation, and regression comparison metadata.
