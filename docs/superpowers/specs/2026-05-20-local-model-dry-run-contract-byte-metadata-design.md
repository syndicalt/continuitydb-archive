# Local Model Dry-Run Contract Byte Metadata Design

## Problem

Local-model dry-run preflight JSON exposes contract fingerprints but not contract byte counts. The standalone contract export and benchmark contract artifact surfaces now expose schema and grammar byte counts, so dry-run preflight output is missing matching size evidence for the same contract strings.

## Goal

Add schema and grammar byte-count metadata to `benchmark-local-model --dry-run` JSON so preflight artifacts expose contract size evidence without requiring files to be written.

## Scope

- Add top-level `schema_bytes` and `grammar_bytes` to dry-run benchmark JSON.
- Compute byte counts from the same in-memory schema and grammar strings used for fingerprints.
- Keep existing dry-run behavior, fingerprints, runtime arguments, and artifact fields unchanged.

## Non-Goals

- Do not change real benchmark report schema in this slice.
- Do not write contract files unless `--contract-dir` or `--artifact-dir` is supplied.
- Do not add JSON Schema or GBNF syntax validation.

## Verification

- Extend `cli_benchmark_local_model_dry_run_outputs_preflight_without_baseline` to assert `schema_bytes` and `grammar_bytes` are present and positive.
- Run the focused dry-run CLI test and the full workspace verification gate.
