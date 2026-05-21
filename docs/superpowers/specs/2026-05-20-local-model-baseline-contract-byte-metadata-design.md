# Local Model Baseline Contract Byte Metadata Design

## Problem

Local-model benchmark dry-runs, contract exports, and contract artifact bundles expose schema and grammar byte counts, but durable benchmark baseline records still preserve only contract fingerprints. A real benchmark run therefore loses contract size evidence at the persisted baseline boundary, even though that baseline is the artifact used for compatibility checks and regression history.

## Goal

Persist response schema and GBNF grammar byte counts in local-model benchmark baselines and expose the same values in real benchmark JSON.

## Scope

- Add `schema_bytes` and `grammar_bytes` to `LocalModelBenchmarkReport`.
- Add `schema_bytes` and `grammar_bytes` to `LocalModelBenchmarkBaseline` with serde defaults for older baseline files.
- Expose accessor methods for embedders that inspect durable baselines.
- Emit top-level `schema_bytes` and `grammar_bytes` from successful `benchmark-local-model` JSON.
- Assert durable JSONL baseline records carry the same byte counts as the CLI benchmark report.

## Non-Goals

- Do not make compatibility filtering depend on byte counts in this slice; fingerprints remain the contract identity gate.
- Do not change the schema or grammar text.
- Do not introduce a new baseline file format version.
- Do not alter dry-run behavior.

## Verification

- Extend `cli_benchmark_local_model_records_baseline` to assert top-level and durable-record `schema_bytes` and `grammar_bytes` are present, positive, and equal.
- Run the focused local-model CLI baseline test.
- Run the full workspace verification gate.
