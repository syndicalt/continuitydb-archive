# Local Model Regression Failure Report Design

## Problem

`benchmark-local-model --fail-on-regression` can reject a regressed local Steward model run before recording a baseline. When callers also provide `--failure-report-path`, the CLI should write the structured benchmark JSON for that rejected run even if no full `--artifact-dir` bundle is requested.

## Constraints

- Do not record the regressed run as a durable baseline.
- Preserve existing artifact-bundle behavior when `--artifact-dir` is present.
- Keep the model boundary proposal-only; this change is reporting behavior only.
- The failure report must include the same regression comparison metadata as bundle reports.
- Without `--artifact-dir`, the report must keep `bundle_manifest` null.

## Test

Add a CLI regression test that:

- Records one passing compatible baseline.
- Re-runs the same candidate/runtime with a regressed executable.
- Passes `--fail-on-regression --failure-report-path <path>` without `--artifact-dir`.
- Expects non-zero exit, exactly one baseline record, and a written report showing the regression.
