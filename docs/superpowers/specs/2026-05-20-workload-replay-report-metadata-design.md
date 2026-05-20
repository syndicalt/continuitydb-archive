# Workload Replay Report Metadata Design

## Context

Replay artifact bundles write `replay-report.json` and a `continuitydb-workload-replay.manifest.json`. The manifest identifies the replay report path and embeds replay evidence, but it does not record the replay report fingerprint or byte count. That leaves archived replay bundles with weaker artifact-level integrity metadata than workload fixture artifacts.

## Design

Add replay report artifact metadata to replay bundle manifests:

- `replay_report_fingerprint` records the FNV-1a fingerprint of the exact `replay-report.json` bytes written before the manifest is generated.
- `replay_report_bytes` records the byte length of that report text.
- The fields live beside `replay_report_path` in `continuitydb-workload-replay.manifest.json`.
- The existing `replay_bundle_manifest` object in `replay-report.json` remains unchanged and continues to describe the manifest artifact itself.

## Scope

This change does not add replay manifest validation yet. It makes replay bundles self-describing enough for future validation and CI artifact integrity checks.

