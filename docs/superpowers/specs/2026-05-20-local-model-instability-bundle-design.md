# Local Model Instability Bundle Design

## Goal

Make `benchmark-local-model --artifact-dir --stability-trials N --fail-on-unstable` preserve a complete artifact bundle when repeated low-temperature outputs drift.

## Context

Fixed-suite failures and baseline regression failures now write artifact bundles before exiting non-zero. Instability failures still return immediately after the repeated-run stability check, before the normal benchmark report and response artifacts are produced. That makes drift failures hard to inspect in CI because only stderr remains.

## Design

When instability is detected and `--artifact-dir` is present, the CLI should run the normal benchmark capture path once, then write:

- `<artifact-dir>/benchmark-report.json`
- `<artifact-dir>/local-model-benchmark.manifest.json`
- contracts, prompts, responses, and nested response manifest where applicable

The report includes `stability.stable = false` and per-case changed-trial metadata. The command still exits non-zero and does not record a baseline.

## Boundaries

- This does not change dry-run preflight behavior.
- This does not record unstable runs as baselines.
- This does not change the error message or non-zero status.
- The additional benchmark capture run happens only when artifact output is needed for an instability failure.

## Testing

Add a CLI test using a counter-backed runner that changes output across stability trials. The test verifies non-zero exit, absent baseline, artifact report creation, root bundle manifest creation, `stability.stable = false`, and response artifact references.
