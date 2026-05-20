# Local Model Response Manifest Content Validation Design

## Problem

`validate-local-model-bundle --artifact-dir` now validates the nested `responses/local-model-responses.manifest.json` file path, byte count, fingerprint, and each captured raw response artifact. That proves the response manifest and response files are internally consistent, but it does not prove the nested response manifest still matches the root benchmark report's `response_artifacts` list.

For Steward model trials, the root benchmark report is the primary evidence object and the nested response manifest is the archive index for raw model stdout. A bundle where both files are individually valid but disagree about response artifact metadata is internally inconsistent.

## Goal

Extend local-model bundle validation so nested response manifests are checked against the archived benchmark report's `response_artifacts` projection.

## Non-Goals

- Do not change response artifact generation.
- Do not change response manifest schema.
- Do not add real model execution or external model dependencies.
- Do not validate changed-case reports in this slice.

## Design

`validate_local_model_bundle_manifest` already parses `benchmark-report.json`. Pass that parsed report into `validate_local_model_response_artifact_manifest`.

After the existing nested response manifest metadata checks pass, parse `responses/local-model-responses.manifest.json` and validate:

- `format == "continuitydb.local_model.responses"`.
- `format_version == 1`.
- `artifacts` exactly equals `benchmark_report["response_artifacts"]`.

Keep per-response raw file validation in `validate_local_model_response_artifact_files`; it should still verify that each captured artifact points inside the bundle response directory and that bytes/fingerprint match the actual archived file.

## Error Boundary

Use one explicit error for content mismatch:

`local model response artifact manifest content mismatch`

Keep existing metadata and raw response errors unchanged so operators can distinguish manifest file corruption, raw response corruption, and root/nested report disagreement.

## Tests

Add feature-gated Unix CLI tests that create a real local-model artifact bundle, mutate `responses/local-model-responses.manifest.json`, refresh the root manifest's nested response-manifest byte count and fingerprint, and assert `validate-local-model-bundle` rejects the bundle with the content-mismatch error.

Test cases:

- Response artifact case-name mismatch.
- Response artifact captured flag mismatch.

These cases prove both identity metadata and capture-state metadata are checked against the benchmark report, while raw response file bytes/fingerprints can still remain valid.
