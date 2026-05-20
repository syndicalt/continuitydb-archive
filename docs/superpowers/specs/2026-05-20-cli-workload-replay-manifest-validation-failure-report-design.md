# CLI Workload Replay Manifest Validation Failure Report Design

## Goal

Write structured failure evidence when `replay-workload --require-manifest` rejects an input workload bundle before replay can start.

## Rationale

Manifest validation happens before workload parsing, ingest, checkout, and comparison. Without a failure report, CI only sees stderr and loses artifact paths and fingerprints that explain which replay fixture failed validation.

## Behavior

- When `--require-manifest` validation fails and `--failure-report-path` is set, write JSON before exiting non-zero.
- Include kernel, artifact directory, optional store/report/replay-artifact paths, fixture artifact paths, current fixture fingerprints, and current fixture byte counts.
- Include `failure.stage = "input_manifest_validation"`.
- Include the validation error message.
- Do not run replay measurement after validation failure.
- Leave existing mismatch failure reports unchanged.

## Non-Goals

- Do not write replay bundle manifests for input validation failures yet.
- Do not make manifest validation mandatory for all replays.
- Do not parse workload cells or checkout requests after validation fails.
