# Workload Report Metadata Validation Design

## Context

Workload bundle manifests now include `workload_report_fingerprint` and `workload_report_bytes` for the root workload report payload. `replay-workload --require-manifest` validates report path and report-owned content fields, but it does not yet validate the new fingerprint or byte metadata.

That leaves the metadata descriptive instead of enforceable. Reproducible storage-engine replay should reject bundles whose manifest-owned report artifact metadata no longer matches the archived report file.

The final `workload-report.json` embeds `bundle_manifest`, and the manifest embeds report metadata. A byte-for-byte fingerprint of the final report would be cyclic. The stable contract is therefore the canonical workload report payload serialized with `bundle_manifest` normalized to `null`, matching the report content before the manifest pointer is inserted.

## Design

Extend `validate_workload_artifact_manifest` to read `workload-report.json`, parse it, canonicalize it with `bundle_manifest: null`, and check before report-content validation:

- `manifest["workload_report_bytes"] == canonical_report_payload_text.len()`
- `manifest["workload_report_fingerprint"] == fnv1a64_fingerprint(&canonical_report_payload_text)`

Use the existing error categories:

- Byte mismatch returns `workload artifact manifest byte count mismatch`.
- Fingerprint mismatch returns `workload artifact manifest fingerprint mismatch`.

## Testing

Add CLI tests that mutate only the manifest metadata after bundle creation:

- `workload_report_bytes = 1` fails `replay-workload --require-manifest`.
- `workload_report_fingerprint = "fnv1a64:0000000000000000"` fails `replay-workload --require-manifest`.
