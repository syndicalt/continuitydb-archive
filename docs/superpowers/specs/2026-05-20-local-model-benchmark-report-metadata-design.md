# Local Model Benchmark Report Metadata Design

## Context

`benchmark-local-model --artifact-dir` writes a root `benchmark-report.json` and a versioned `local-model-benchmark.manifest.json`. The manifest currently records the benchmark report path but not the report artifact fingerprint or byte count.

Replay bundles already include report fingerprint and byte metadata. Local model benchmark bundles should follow the same integrity pattern so archived Steward model evaluations can verify the root report artifact without reparsing or trusting path-only references.

## Design

Extend local model benchmark bundle manifests with:

- `benchmark_report_fingerprint`
- `benchmark_report_bytes`

The manifest writer will read the already-written `benchmark-report.json`, compute the existing FNV-1a fingerprint with `local_model_contract_fingerprint`, and store the byte count beside `benchmark_report_path`.

## Surfaces

Expose the metadata only in `local-model-benchmark.manifest.json`. The existing `bundle_manifest` summary in the root report continues to describe the manifest artifact itself.

## Testing

Extend the existing local model artifact bundle CLI test to assert that the bundle manifest records:

- `benchmark_report_fingerprint` with an `fnv1a64:` prefix.
- `benchmark_report_bytes > 0`.

