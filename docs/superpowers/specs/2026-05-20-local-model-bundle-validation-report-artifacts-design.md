# Local Model Bundle Validation Report Artifacts Design

## Problem

`validate-local-model-bundle --artifact-dir` validates archived local Steward benchmark bundles and prints validation evidence to stdout, but it cannot write the successful validation JSON to an archiveable report file. Workload bundle validation already supports durable validation reports, so local-model bundle validation is behind the same CI artifact standard.

## Goal

Add a `--report-path` option to `validate-local-model-bundle` so successful validation evidence can be written to a durable JSON artifact while preserving stdout output.

## Scope

- Add `--report-path` to the feature-gated `validate-local-model-bundle` command.
- Write the exact successful validation JSON emitted to stdout to the requested path.
- Include the requested report path in the validation JSON for auditability.
- Update README and roadmap.

## Non-Goals

- Do not add failure reports in this slice.
- Do not change bundle validation semantics.
- Do not change benchmark bundle generation.

## Verification

- Add a CLI acceptance test that validates a dry-run local-model bundle with `--report-path`, parses both stdout and the written file, and asserts they match.
- Run the focused local-model CLI test.
- Run the full workspace verification gate.
