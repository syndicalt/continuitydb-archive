# Local Model Response Manifest Content Diagnostics Design

## Problem

`validate-local-model-bundle --artifact-dir` rejects archived response artifact manifests when the manifest content no longer matches the benchmark report. The current error message says only `local model response artifact manifest content mismatch`, which forces operators to diff the nested response manifest and benchmark report manually.

## Goal

When response artifact manifest content validation fails, report the specific mismatched field so CI and operators can diagnose bundle drift directly from stderr.

## Scope

- Preserve the existing strict validation semantics.
- Add deterministic mismatch labels for root metadata and response artifact list mismatches.
- Keep the validation command output shape unchanged on success.
- Update roadmap and README scope.

## Non-Goals

- Do not add a new validation report format.
- Do not relax response artifact manifest validation.
- Do not change local-model benchmark bundle generation.

## Proposed Behavior

For response artifact manifest content validation:

- `format` mismatch reports `local model response artifact manifest content mismatch: format`.
- `format_version` mismatch reports `local model response artifact manifest content mismatch: format_version`.
- `artifacts` mismatch reports `local model response artifact manifest content mismatch: artifacts`.

## Verification

- Add a CLI regression test that tampers a response artifact manifest field and expects the field-specific mismatch text.
- Run the focused CLI test with `--features local-model`.
- Run the full workspace verification gate.
