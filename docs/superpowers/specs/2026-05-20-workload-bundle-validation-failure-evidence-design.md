# Workload Bundle Validation Failure Evidence Design

## Problem

`validate-workload-bundle --failure-report-path` records the validation failure stage and message, but it currently sets `workload_artifacts` to `null`. When a bundle is rejected because a fixture fingerprint changed, CI loses the current artifact paths, byte counts, and fingerprints needed to diagnose which archived file was inspected.

## Goal

Record best-effort workload fixture metadata in validation failure reports.

For available files, failure reports should include:

- `workload_artifacts.cells_path`
- `workload_artifacts.cells_fingerprint`
- `workload_artifacts.cells_bytes`
- `workload_artifacts.checkout_request_path`
- `workload_artifacts.checkout_request_fingerprint`
- `workload_artifacts.checkout_request_bytes`

## Design

Add a best-effort helper that reads `workload-cells.json` and `checkout-request.json` from the artifact directory. If both files can be read, compute the same FNV fingerprints used by successful validation and replay failure reports. If either file is unavailable, return `null` so the failure report still preserves the validation error without inventing metadata.

## Non-Goals

- Do not change validation rules.
- Do not mark invalid artifact metadata as trusted.
- Do not mutate workload bundles.
- Do not add a validation manifest for validation reports.

## Tests

Add a CLI test that tampers with `workload-cells.json`, runs:

```sh
continuitydb validate-workload-bundle --artifact-dir <dir> --failure-report-path <failure.json>
```

and asserts the failure report includes current artifact paths, fingerprints, and byte counts for both workload fixture files.
