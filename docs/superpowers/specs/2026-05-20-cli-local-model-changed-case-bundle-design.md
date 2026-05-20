# CLI Local Model Changed-Case Bundle Design

## Problem

`benchmark-local-model --changed-case-report-path` writes a compact drift report, but artifact bundles created with `--artifact-dir --compare-baseline` still require consumers to parse the full benchmark report unless the caller also selects a separate changed-case report path.

## Design

- When `benchmark-local-model --artifact-dir` is used with baseline comparison, write `changed-cases.json` inside the artifact directory unless `--changed-case-report-path` explicitly selects another path.
- Include the effective changed-case report path in stdout/full benchmark JSON.
- Include the effective changed-case report path in `local-model-benchmark.manifest.json`.
- Keep no-comparison dry-run and normal runs unchanged except for null manifest metadata.

## Test

- Add a feature-gated CLI test that records a passing baseline, reruns with a changed passing response using `--compare-baseline --artifact-dir`, and asserts:
  - `changed-cases.json` exists in the artifact directory.
  - stdout reports `changed_case_report_path`.
  - the root bundle manifest reports `changed_case_report_path`.
  - the changed-case report includes the expected changed-case count.
