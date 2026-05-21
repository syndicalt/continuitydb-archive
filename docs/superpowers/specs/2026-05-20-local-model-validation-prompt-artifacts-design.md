# Local Model Validation Prompt Artifacts Design

## Problem

Local-model benchmark bundles archive deterministic prompt artifacts and record prompt paths, fingerprints, and byte counts in `benchmark-report.json` and `local-model-benchmark.manifest.json`. `validate-local-model-bundle` validates benchmark reports, changed-case reports, and raw response artifacts, but it does not validate archived prompt files. A bundle can pass validation after a prompt file is tampered without updating the report or manifest.

## Goal

Validate archived local-model prompt artifacts during successful bundle validation and expose the validated prompt artifact list in validation JSON.

## Scope

- Check that the bundle manifest `prompt_artifacts` projection matches the benchmark report `prompt_artifacts` projection.
- For each prompt artifact, require the prompt path to stay under `artifact_dir/prompts`.
- Validate each prompt file's current byte count and fingerprint against the archived metadata.
- Return the validated prompt artifact list in successful `validate-local-model-bundle` JSON.

## Non-Goals

- Do not regenerate prompts during validation.
- Do not validate prompt semantic content beyond archived bytes and fingerprints.
- Do not change prompt artifact creation.

## Verification

- Add a CLI test that creates a dry-run local-model bundle, tampers a prompt file with same-length replacement content, and asserts `validate-local-model-bundle` fails with a prompt fingerprint mismatch.
- Run focused local-model CLI test and the full workspace verification gate.
