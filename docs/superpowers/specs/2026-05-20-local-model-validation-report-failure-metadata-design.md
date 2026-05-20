# Local Model Validation Report Failure Metadata Design

## Problem

`validate-local-model-bundle --failure-report-path` includes best-effort manifest metadata, but `benchmark_report` remains `null`. When a local Steward benchmark bundle is rejected, CI artifacts should identify the archived `benchmark-report.json` bytes that were inspected.

## Goal

Add best-effort benchmark report metadata to local-model bundle validation failure reports.

## Scope

- Read `benchmark-report.json` if it exists.
- Prefer the canonical manifest payload text when the report can be parsed and normalized.
- Fall back to raw file bytes when parsing or normalization fails.
- Record report path, fingerprint, and byte count.
- Return `null` when the report cannot be read.

## Non-Goals

- Do not add changed-case or response manifest failure metadata in this slice.
- Do not change validation rules.
- Do not change successful validation output.

## Verification

- Extend the existing CLI failure-report test to assert `benchmark_report` metadata is populated.
- Run the focused local-model CLI test.
- Run the full workspace verification gate.
