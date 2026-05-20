# Local Model Instability Failure Report Design

## Problem

`benchmark-local-model --stability-trials --fail-on-unstable` can reject a local Steward model before recording a baseline when repeated runs drift. Operators need the structured benchmark JSON at `--failure-report-path` even when they do not request a full `--artifact-dir` bundle.

## Constraints

- Do not record an unstable run as a durable baseline.
- Preserve existing instability artifact bundle behavior.
- Include stability drift metadata in the written report.
- Without `--artifact-dir`, keep `bundle_manifest` null.
- This is reporting only; the Steward model remains proposal-only.

## Test

Add a CLI test that:

- Uses an executable fixture that changes one proposal rationale after stability trials.
- Runs `benchmark-local-model --stability-trials 2 --fail-on-unstable --failure-report-path <path>`.
- Expects non-zero exit, no baseline file, and a written report with `stability.stable = false`.
