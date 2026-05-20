# Local Model Validation Response Artifact Failure Metadata Design

## Problem

`validate-local-model-bundle --failure-report-path` preserves the response artifact manifest metadata when a local Steward benchmark bundle is rejected, but it does not report current per-response artifact evidence. If validation fails because a raw response file was tampered, CI artifacts identify the manifest but not the response file bytes and fingerprint that caused the rejection.

## Goal

Add best-effort response artifact file metadata to local-model bundle validation failure reports.

## Scope

- Parse `responses/local-model-responses.manifest.json` when present and readable.
- Emit `response_artifacts` as an array containing the manifest-declared case name, capture state, response path, current response fingerprint, and current response byte count.
- Read only response paths inside the bundle's `responses/` directory.
- Return `null` when the response artifact manifest is missing, unreadable, malformed, or lacks an artifact list.

## Non-Goals

- Do not validate response artifact content from the failure metadata helper.
- Do not follow response paths outside the bundle response directory.
- Do not change successful validation output shape.

## Verification

- Add a CLI test that creates a real local-model bundle, tampers one raw response artifact without changing its length, and runs `validate-local-model-bundle --failure-report-path`.
- Assert the failure report includes `response_artifacts` with the current response path, fingerprint, and byte count.
- Run focused local-model CLI test and the full workspace verification gate.
