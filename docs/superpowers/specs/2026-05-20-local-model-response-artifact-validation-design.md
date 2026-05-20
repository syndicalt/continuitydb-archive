# Local Model Response Artifact Validation Design

## Context

Local model benchmark bundles can archive raw per-case model responses under `responses/`. The nested `responses/local-model-responses.manifest.json` records each captured response path, fingerprint, and byte count.

`validate-local-model-bundle --artifact-dir` now validates the root bundle manifest and the nested response manifest metadata, but it does not yet validate the individual response files listed inside the nested manifest.

## Design

Extend response manifest validation to parse `responses/local-model-responses.manifest.json` and enforce each captured response artifact:

- response manifest format is `continuitydb.local_model.responses`
- response manifest format version is `1`
- captured artifacts have `response_path`, `response_bytes`, and `response_fingerprint`
- each captured `response_path` is inside `<artifact_dir>/responses/`
- each captured response file byte count and fingerprint match the nested manifest

Non-captured artifacts should continue to require no file validation.

On failure, return non-zero with specific error categories:

- `local model response artifact manifest path mismatch`
- `local model response artifact byte count mismatch`
- `local model response artifact fingerprint mismatch`

## Testing

Add feature-gated Unix CLI tests that create a real local-model benchmark artifact bundle, then validate:

- tampering a captured raw response file causes a fingerprint mismatch
- changing a captured response byte count in the nested response manifest causes a byte-count mismatch after refreshing the root response-manifest metadata
- changing a captured response fingerprint in the nested response manifest causes a fingerprint mismatch after refreshing the root response-manifest metadata
- changing a captured response path outside the response artifact directory causes a path mismatch after refreshing the root response-manifest metadata
