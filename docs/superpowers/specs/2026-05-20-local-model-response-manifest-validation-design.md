# Local Model Response Manifest Validation Design

## Context

Local model benchmark bundles can include raw per-case response artifacts under `responses/` plus a nested `responses/local-model-responses.manifest.json`. The root `local-model-benchmark.manifest.json` records `response_artifact_manifest` with path, fingerprint, and byte count.

`validate-local-model-bundle --artifact-dir` currently validates the root benchmark report and optional changed-case report metadata. It does not yet validate the nested response artifact manifest, leaving raw-response archive metadata descriptive rather than enforceable.

## Design

Extend `validate-local-model-bundle --artifact-dir` to validate the nested response artifact manifest when present:

- `manifest["response_artifact_manifest"]["manifest_path"] == <artifact_dir>/responses/local-model-responses.manifest.json`
- `manifest["response_artifact_manifest"]["manifest_bytes"] == response_manifest_text.len()`
- `manifest["response_artifact_manifest"]["manifest_fingerprint"] == fnv1a64_fingerprint(response_manifest_text)`

If no response artifact manifest is present, validation should continue to succeed and emit `response_artifact_manifest: null`.

On success, include `response_artifact_manifest` in the validation JSON. On failure, return non-zero with specific error categories:

- `local model benchmark manifest response artifact manifest path mismatch`
- `local model benchmark manifest response artifact manifest byte count mismatch`
- `local model benchmark manifest response artifact manifest fingerprint mismatch`

## Testing

Add feature-gated Unix CLI tests that create a real local-model benchmark artifact bundle, then validate:

- untouched response manifest metadata succeeds and is emitted
- response manifest path mismatch fails
- response manifest byte-count mismatch fails
- response manifest fingerprint mismatch fails
