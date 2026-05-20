# CLI Workload Replay Manifest Report Path Validation Design

## Problem

`replay-workload --require-manifest` validates fixture paths and fingerprints, but the required workload bundle manifest also records the archived `workload-report.json` path. A tampered manifest can point that report field somewhere else while the replay still succeeds.

## Desired Behavior

When `--require-manifest` is set, replay validates that `workload_report_path` is exactly `<artifact-dir>/workload-report.json`.

## Scope

- Reject mismatched manifest report paths before replaying workload artifacts.
- Preserve the existing fixture fingerprint validation behavior.
- Keep the validation deterministic and local to the existing manifest validation function.

## Out of Scope

- Verifying the report file fingerprint.
- Changing workload bundle format version.
- Rewriting existing artifact bundle layouts.
