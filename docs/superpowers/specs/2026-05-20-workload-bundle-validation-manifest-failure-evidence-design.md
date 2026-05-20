# Workload Bundle Validation Manifest Failure Evidence Design

## Problem

Direct workload bundle validation failure reports include fixture metadata, but still set `manifest` to `null`. When validation fails because a manifest field is wrong, CI should still see which manifest file was inspected and its current fingerprint and byte count.

## Goal

Record best-effort manifest metadata in `validate-workload-bundle --failure-report-path` output:

- `manifest.manifest_path`
- `manifest.manifest_fingerprint`
- `manifest.manifest_bytes`

## Design

Add a helper that reads `continuitydb-workload.manifest.json` from the artifact directory. If the file can be read, return the same metadata shape as successful validation. If it cannot be read, return `null` so missing-manifest failures still report the validation error without inventing evidence.

## Non-Goals

- Do not validate the manifest in the failure metadata helper.
- Do not parse the manifest as JSON for this evidence path.
- Do not change any existing validation rule.
- Do not change successful validation output.

## Tests

Add a CLI test that corrupts a manifest field, runs:

```sh
continuitydb validate-workload-bundle --artifact-dir <dir> --failure-report-path <failure.json>
```

and asserts the failure report includes current manifest path, fingerprint, and byte count.
