# Local Model Failure Bundle Design

## Goal

Make `benchmark-local-model --artifact-dir --fail-on-failed-cases` preserve a complete artifact bundle when fixed evaluation cases fail.

## Context

Successful real runs and dry-runs with `--artifact-dir` write contracts, prompts, reports, response artifacts, and a root bundle manifest. Fixed-suite failures currently exit before the command wrapper writes the artifact-directory report and bundle manifest, so CI loses the most useful failure evidence unless callers also know to provide `--failure-report-path`.

## Design

When a real benchmark run fails the fixed evaluation gate and `--artifact-dir` is present, the CLI writes:

- `<artifact-dir>/benchmark-report.json`
- `<artifact-dir>/local-model-benchmark.manifest.json`
- `<artifact-dir>/contracts/*`
- `<artifact-dir>/prompts/*`
- `<artifact-dir>/responses/*`
- `<artifact-dir>/responses/local-model-responses.manifest.json`

The benchmark report includes `bundle_manifest` metadata just like successful bundles. The command still exits non-zero and does not append a baseline. If `--failure-report-path` is also present, it receives the same final JSON as the artifact report.

## Boundaries

- This does not change dry-run behavior.
- This does not change stability failure behavior.
- This does not record failed fixed-suite runs as baselines.
- The artifact directory remains the default bundle location; explicit `--contract-dir`, `--prompt-dir`, and `--response-dir` continue to override their artifact kinds.

## Testing

Add a CLI test that runs a deliberately incomplete local model response with `--artifact-dir --fail-on-failed-cases`. The process must fail, the baseline must not exist, and the artifact directory must contain the report and root bundle manifest with failed-case details.
