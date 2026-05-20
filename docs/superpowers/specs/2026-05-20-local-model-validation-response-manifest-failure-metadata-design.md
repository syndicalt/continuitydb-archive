# Local Model Validation Response Manifest Failure Metadata Design

## Problem

`validate-local-model-bundle --failure-report-path` preserves manifest, benchmark report, and changed-case report evidence when local Steward benchmark bundle validation rejects an artifact. It still emits `response_artifact_manifest: null` even when the rejected bundle contains `responses/local-model-responses.manifest.json`.

## Goal

Add best-effort response artifact manifest metadata to local-model bundle validation failure reports.

## Scope

- Read `responses/local-model-responses.manifest.json` when present.
- Report `manifest_path`, `manifest_fingerprint`, and `manifest_bytes`.
- Return `null` when the response artifact manifest does not exist or cannot be read.
- Do not validate response manifest content in the failure metadata helper.

## Non-Goals

- Do not validate individual response artifacts from the failure metadata helper.
- Do not change successful validation output shape.
- Do not change benchmark bundle creation semantics.

## Verification

- Add a CLI test that creates a real local-model bundle with response artifacts, tampers the bundle manifest benchmark report byte count, and runs `validate-local-model-bundle --failure-report-path`.
- Assert the failure report includes response artifact manifest path, fingerprint, and byte count.
- Run focused local-model CLI test and the full workspace verification gate.
