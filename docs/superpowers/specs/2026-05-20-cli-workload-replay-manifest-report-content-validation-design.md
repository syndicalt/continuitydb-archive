# CLI Workload Replay Manifest Report Content Validation Design

## Problem

`replay-workload --require-manifest` validates that the manifest points at `<artifact-dir>/workload-report.json`, but it does not verify that the archived report content still matches the manifest. A workload report can drift while the manifest and replay fixtures remain intact.

## Desired Behavior

When `--require-manifest` is set, replay validates that manifest-owned report fields match the archived `workload-report.json` fields:

- `kernel`
- `store_path`
- `artifact_dir`
- `baseline_path`
- `baseline_label`
- `baseline_comparison`
- `lookup_plan`
- `workload_artifacts`
- `workload`

## Scope

- Reject self-inconsistent workload bundles before replaying fixtures.
- Preserve existing path, byte-count, fingerprint, artifact-directory, report-path, and workload-summary validation.
- Avoid comparing report-only fields such as `bundle_manifest`.

## Out of Scope

- Adding report fingerprints to the manifest.
- Changing workload bundle format version.
- Validating replay output bundle report fields.
