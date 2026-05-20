# Local Model Report Metadata Validation Design

## Context

Local model benchmark bundle manifests include `benchmark_report_fingerprint` and `benchmark_report_bytes`. The final `benchmark-report.json` embeds `bundle_manifest`, while the manifest embeds benchmark report metadata, so the stable non-cyclic contract is the benchmark report payload serialized with `bundle_manifest` normalized to `null`.

Workload artifact bundles now validate this contract through `replay-workload --require-manifest`. Local model benchmark bundles have the metadata but no operator command that verifies archived bundle integrity after creation.

## Design

Add a feature-gated CLI command:

```sh
continuitydb validate-local-model-bundle --artifact-dir <dir>
```

The command reads `<dir>/local-model-benchmark.manifest.json` and `<dir>/benchmark-report.json`, validates:

- manifest format is `continuitydb.local_model.benchmark_bundle`
- manifest format version is `1`
- `benchmark_report_path` equals `<dir>/benchmark-report.json`
- `benchmark_report_bytes` equals the canonical benchmark report payload byte length
- `benchmark_report_fingerprint` equals the canonical benchmark report payload fingerprint

The canonical benchmark report payload is the parsed `benchmark-report.json` serialized with `bundle_manifest` set to `null`.

On success, print structured JSON with the artifact directory, manifest path/fingerprint/bytes, and benchmark report path/fingerprint/bytes. On failure, return non-zero with the same error categories as workload manifest validation:

- `local model benchmark manifest report path mismatch`
- `local model benchmark manifest byte count mismatch`
- `local model benchmark manifest fingerprint mismatch`

## Testing

Add CLI tests that create a dry-run local-model benchmark bundle, mutate only the manifest metadata, and validate:

- success for an untouched dry-run bundle
- `benchmark_report_bytes = 1` fails validation
- `benchmark_report_fingerprint = "fnv1a64:0000000000000000"` fails validation
