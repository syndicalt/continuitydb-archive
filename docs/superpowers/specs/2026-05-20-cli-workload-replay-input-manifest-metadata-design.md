# CLI Workload Replay Input Manifest Metadata Design

## Goal

Record which workload bundle manifest was validated during `replay-workload --require-manifest`.

## Rationale

Manifest validation rejects tampered replay fixtures, but replay reports also need to prove which manifest was enforced. Storage-engine trial artifacts should be self-contained enough for CI and humans to verify the source bundle identity without reading the original workload directory.

## Behavior

- When `--require-manifest` is set, include `input_bundle_manifest` in replay JSON.
- Include the input manifest path, FNV-1a fingerprint, and byte count.
- Copy the same metadata into `continuitydb-workload-replay.manifest.json` when `--replay-artifact-dir` is used.
- Leave `input_bundle_manifest` as `null` when manifest validation is not requested.

## Non-Goals

- Do not make manifest validation mandatory for every replay.
- Do not change workload bundle format version.
- Do not copy the input manifest file into the replay artifact directory.
