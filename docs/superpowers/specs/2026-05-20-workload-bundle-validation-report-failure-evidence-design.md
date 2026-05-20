# Workload Bundle Validation Report Failure Evidence Design

## Problem

Direct workload bundle validation failure reports now include manifest and fixture evidence, but still set `workload_report` to `null`. When a bundle fails validation because report metadata is stale or manifest-owned report fields drifted, CI should see which `workload-report.json` bytes were inspected.

## Goal

Record best-effort workload report metadata in `validate-workload-bundle --failure-report-path` output:

- `workload_report.report_path`
- `workload_report.report_fingerprint`
- `workload_report.report_bytes`

## Design

Add a helper that reads `workload-report.json` from the artifact directory. If the file can be read, compute the same canonical report payload used for successful validation by normalizing `bundle_manifest` to `null`, then return path, fingerprint, and byte count. If canonicalization fails, return raw file metadata using the same path and fingerprint fields so the failure report still identifies the inspected file bytes. If the file cannot be read, return `null`.

## Non-Goals

- Do not validate report contents in the failure evidence helper.
- Do not mutate the workload bundle.
- Do not change successful validation output.
- Do not change existing validation rules.

## Tests

Add a CLI test that corrupts manifest report metadata, runs:

```sh
continuitydb validate-workload-bundle --artifact-dir <dir> --failure-report-path <failure.json>
```

and asserts the failure report includes current workload report path, fingerprint, and byte count.
