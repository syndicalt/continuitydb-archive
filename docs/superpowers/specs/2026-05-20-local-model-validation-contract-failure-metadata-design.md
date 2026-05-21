# Local Model Validation Contract Failure Metadata Design

## Problem

`validate-local-model-bundle --failure-report-path` preserves best-effort evidence for manifests, benchmark reports, changed-case reports, response manifests, and response artifacts. Contract artifacts are now validated, but failure reports do not preserve the inspected schema and grammar artifact evidence when bundle validation fails.

## Goal

Preserve best-effort local-model contract artifact metadata in validation failure reports so CI and operators can diagnose tampered or missing response schema and grammar files without manually inspecting the bundle.

## Scope

- Add a `contract_artifacts` section to local-model bundle validation failure reports.
- Return `null` when `benchmark-report.json` is missing, unparsable, lacks `contract_artifacts`, or no contract artifacts were archived.
- When present, report current schema and grammar paths, fingerprints, and byte counts from the files on disk.
- Preserve successful validation behavior and existing failure report fields.

## Non-Goals

- Do not add contract artifact byte counts to the benchmark report or bundle manifest schema.
- Do not change contract artifact validation semantics.
- Do not validate JSON Schema or GBNF syntax in this slice.

## Verification

- Add a CLI test that creates a dry-run local-model benchmark bundle, tampers the archived response schema file, runs `validate-local-model-bundle --failure-report-path`, and asserts the failure report contains current contract artifact metadata for both schema and grammar files.
- Run the focused local-model CLI test and the full workspace verification gate.
