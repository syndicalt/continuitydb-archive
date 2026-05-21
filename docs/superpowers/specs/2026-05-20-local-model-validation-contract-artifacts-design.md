# Local Model Validation Contract Artifacts Design

## Problem

Local-model benchmark bundles archive response contract artifacts under `contracts/` and record schema and grammar paths plus fingerprints in `benchmark-report.json` and `local-model-benchmark.manifest.json`. `validate-local-model-bundle` does not validate those contract files. A bundle can pass validation after the JSON Schema or GBNF grammar artifact is tampered.

## Goal

Validate archived local-model contract artifacts during bundle validation and expose the validated contract artifact metadata in validation JSON.

## Scope

- Check that the bundle manifest `contract_artifacts` projection matches the benchmark report `contract_artifacts` projection.
- Return `null` when no contract artifacts are present.
- Require schema and grammar paths to stay under `artifact_dir/contracts`.
- Validate schema and grammar file fingerprints against archived metadata.
- Return the validated contract artifact metadata in successful `validate-local-model-bundle` JSON.

## Non-Goals

- Do not validate JSON Schema or GBNF syntax beyond archived byte fingerprint integrity.
- Do not regenerate contract artifacts during validation.
- Do not add byte-count metadata to existing contract artifact records.

## Verification

- Add a CLI test that creates a dry-run local-model bundle, tampers the archived response schema file, and asserts `validate-local-model-bundle` fails with a contract schema fingerprint mismatch.
- Run focused local-model CLI test and the full workspace verification gate.
