# CLI Local Model Changed-Case Requirement Design

## Problem

`benchmark-local-model --changed-case-report-path` is only meaningful when a baseline comparison exists. Without `--compare-baseline` or `--fail-on-regression`, the report would describe a non-comparison and could mislead CI consumers.

## Design

- Reject explicit `--changed-case-report-path` unless baseline comparison is enabled.
- Treat `--fail-on-regression` as enabling baseline comparison through the existing CLI option normalization.
- Keep automatic `--artifact-dir --compare-baseline` changed-case reports unchanged.

## Test

- Add a feature-gated CLI test that runs `benchmark-local-model --dry-run --changed-case-report-path` without comparison and asserts a non-zero exit plus a clear error message.
