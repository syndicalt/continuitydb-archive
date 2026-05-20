# Workload Report Metadata Design

## Context

`measure-workload --artifact-dir` writes a root `workload-report.json` and a versioned `continuitydb-workload.manifest.json`. The manifest records `workload_report_path`, but it does not identify the report artifact by fingerprint or byte count.

Replay bundle manifests already record replay report metadata, and local model benchmark bundle manifests now record benchmark report metadata. Workload measurement bundles should use the same integrity pattern so archived storage-engine workload evidence can verify the root report artifact directly.

## Design

Extend workload bundle manifests with:

- `workload_report_fingerprint`
- `workload_report_bytes`

The manifest writer will read the already-written `workload-report.json`, compute the existing FNV-1a fingerprint with `fnv1a64_fingerprint`, and store the byte count beside `workload_report_path`.

## Surfaces

Expose the metadata only in `continuitydb-workload.manifest.json`. The existing `bundle_manifest` summary in the root report continues to describe the manifest artifact itself.

## Testing

Extend `cli_measure_workload_artifact_dir_writes_bundle` to assert that the bundle manifest records:

- `workload_report_fingerprint` with an `fnv1a64:` prefix.
- `workload_report_bytes > 0`.

