# Workload Bundle Validation Command Design

## Problem

Workload artifact bundles can be validated today only by running `replay-workload --require-manifest`. That couples archive integrity checks to replay execution, store setup, and kernel choice. Storage-engine CI should be able to validate an archived workload bundle manifest directly before replaying or comparing engines.

## Goal

Add a direct CLI command:

```sh
continuitydb validate-workload-bundle --artifact-dir <dir>
```

The command validates `continuitydb-workload.manifest.json`, `workload-report.json`, `workload-cells.json`, and `checkout-request.json` using the same checks already enforced by `replay-workload --require-manifest`.

## Design

Reuse the existing workload manifest validation path instead of duplicating validation rules. Add a small `WorkloadBundleValidation` result that carries:

- validated manifest metadata
- canonical workload report metadata
- workload fixture artifact metadata

The command prints structured JSON so CI can archive the validation result.

## Non-Goals

- Do not execute workload replay.
- Do not add new manifest fields.
- Do not change workload bundle generation.
- Do not change the existing `replay-workload --require-manifest` behavior.

## Tests

Add CLI tests that:

- create a workload artifact bundle with `measure-workload --artifact-dir`
- validate it with `validate-workload-bundle --artifact-dir`
- assert the output identifies the manifest, report, and fixture metadata
- tamper with `workload-cells.json`
- assert `validate-workload-bundle` rejects the bundle with the existing manifest fingerprint mismatch error
