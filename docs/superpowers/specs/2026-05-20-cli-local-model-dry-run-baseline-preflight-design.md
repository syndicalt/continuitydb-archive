# CLI Local Model Dry-Run Baseline Preflight Design

Add read-only compatible-baseline preflight metadata to `benchmark-local-model --dry-run`.

## Problem

`benchmark-local-model --dry-run` now exposes enough contract metadata to predict whether a real benchmark run can compare against a previous compatible baseline: runtime manifest, response schema version, evaluation suite fingerprint, schema fingerprint, grammar fingerprint, and prompt fingerprint.

However, a dry-run with `--compare-baseline` only prints configuration. Operators cannot know whether an expensive local model run will actually compare against a compatible prior baseline or silently record without a previous comparison.

## Goal

When `benchmark-local-model --dry-run --compare-baseline` is supplied, the CLI should inspect the existing baseline file read-only and report whether a compatible baseline exists for the dry-run configuration.

The output should include:

- `baseline_preflight.compared = true`
- `baseline_preflight.compatible_baseline_found`
- `baseline_preflight.previous_recorded_at` when found, otherwise `null`

Compatibility must match the same dimensions used by real benchmark regression gates:

- candidate model ID
- candidate role
- response schema version
- evaluation suite fingerprint
- schema fingerprint
- grammar fingerprint
- prompt fingerprint
- runtime manifest

## Boundaries

- Do not execute a local model during dry-run.
- Do not create or mutate a missing baseline file during dry-run preflight.
- Do not weaken real benchmark compatibility logic.
- Keep default dry-run behavior unchanged when `--compare-baseline` is absent.

## Acceptance Criteria

- A dry-run with `--compare-baseline` and a compatible existing baseline reports `compatible_baseline_found = true` and `previous_recorded_at`.
- The dry-run does not append to the baseline file.
- A dry-run with `--compare-baseline` and no baseline file reports `compatible_baseline_found = false` without creating the file.
- README and roadmap document the new dry-run preflight.
