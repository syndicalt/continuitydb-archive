# CLI Workload Replay Manifest Validation Failure Bundle Design

## Goal

Archive replay artifact bundles when `replay-workload --require-manifest` rejects an input workload bundle before replay starts.

## Rationale

The standalone `--failure-report-path` captures input-manifest validation errors, but CI workflows that standardize on `--replay-artifact-dir` should receive the same structured evidence under the replay artifact bundle directory.

## Behavior

- When input manifest validation fails and `--replay-artifact-dir` is set, write `replay-report.json`.
- Write `continuitydb-workload-replay.manifest.json` for the failed replay bundle.
- Include failure stage, failure message, artifact paths, fixture fingerprints, and fixture byte counts.
- Leave measurement-specific fields as `null` because replay never starts.
- Preserve the existing standalone `--failure-report-path` behavior.

## Non-Goals

- Do not copy or mutate the source workload bundle.
- Do not parse workload cells or checkout requests after validation fails.
- Do not attempt replay comparison for validation failures.
