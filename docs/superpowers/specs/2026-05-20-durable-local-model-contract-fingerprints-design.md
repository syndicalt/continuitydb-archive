# Durable Local Model Contract Fingerprints Design

## Problem

CLI dry-run and contract export now include deterministic JSON Schema and GBNF grammar fingerprints, but benchmark reports and durable baselines still persist only `response_schema_version`. A baseline recorded under one grammar could be compared against a later run using changed contract text as long as the numeric schema version stayed unchanged.

## Goal

Persist local-model response schema and grammar fingerprints on benchmark reports and durable baselines, expose them through public accessors, and include them in compatible-baseline matching.

## Non-Goals

- Do not change schema or grammar text.
- Do not remove legacy baseline compatibility.
- Do not change benchmark scoring.
- Do not add cryptographic signatures.

## Design

Add deterministic contract fingerprint fields to:

- `LocalModelBenchmarkReport`
- `LocalModelBenchmarkBaseline`

Fields:

- `schema_fingerprint`
- `grammar_fingerprint`

Populate them during `LocalModelBenchmark::run()` from the current local model response schema and grammar text. Existing legacy baseline JSON remains readable by defaulting missing fields to empty strings.

Update `latest_compatible_local_model_benchmark_baseline()` to require matching schema and grammar fingerprints in addition to candidate identity, response schema version, evaluation-suite fingerprint, and runtime manifest.

## Acceptance Criteria

- Benchmark reports expose schema and grammar fingerprints.
- Durable baselines created from reports preserve those fingerprints.
- Legacy JSON without the new fields decodes with empty fingerprints.
- Compatible-baseline lookup rejects otherwise matching records with different schema or grammar fingerprints.
- CLI benchmark JSON includes the durable fingerprints from the baseline record.
