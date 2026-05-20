# CLI Workload Replay Manifest Workload Summary Validation Design

## Problem

`replay-workload --require-manifest` validates bundle paths and fixture fingerprints, but it can still accept a workload bundle manifest whose top-level `workload` summary no longer describes the archived `workload-cells.json` fixture.

## Desired Behavior

When `--require-manifest` is set, replay validates that manifest `workload` exactly matches the `summary` field in `workload-cells.json`.

## Scope

- Reject self-inconsistent workload bundle manifests before replaying fixtures.
- Preserve existing artifact-directory, report-path, fixture-path, and fixture-fingerprint validation.
- Keep validation deterministic and local to the required manifest validation path.

## Out of Scope

- Recomputing the workload summary from decoded StateCells.
- Validating kernel or store path fields.
- Changing workload bundle format version.
