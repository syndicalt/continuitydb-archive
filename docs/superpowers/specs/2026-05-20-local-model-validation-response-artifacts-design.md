# Local Model Validation Response Artifacts Design

## Problem

Successful `validate-local-model-bundle` output reports benchmark report metadata, changed-case report metadata, and response artifact manifest metadata. It does not return the validated raw response artifact list, even though the benchmark report and response manifest validation already prove those artifacts are part of the archived bundle.

## Goal

Expose validated raw response artifact metadata in successful local-model bundle validation JSON.

## Scope

- Add `response_artifacts` to `validate-local-model-bundle` success output.
- Preserve the benchmark report's response artifact projection after bundle validation succeeds.
- Return `null` for dry-run bundles or bundles without response artifacts.
- Keep failure-report behavior unchanged.

## Non-Goals

- Do not add new artifact files.
- Do not change the response artifact manifest format.
- Do not change local-model benchmark bundle creation.

## Verification

- Add a CLI test that creates a real local-model bundle, validates it, and asserts `response_artifacts` contains captured raw response path, fingerprint, byte count, and case name metadata.
- Run focused local-model CLI test and the full workspace verification gate.
