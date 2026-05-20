# Local Model Contract Version Baselines Design

## Goal

Persist the Steward local-model response contract version in benchmark reports and durable baselines.

## Context

Local model baselines now preserve candidate identity, evaluation results, runtime manifests, and timestamps. The CLI can also export the JSON Schema and GBNF grammar. The remaining reproducibility gap is that a baseline does not say which local-model response contract version the benchmark decoded and scored.

As the Steward schema evolves, durable artifacts need to distinguish old baselines from current ones without relying on external notes.

## Architecture

Add `response_schema_version: u32` to:

- `LocalModelBenchmarkReport`
- `LocalModelBenchmarkBaseline`

Populate new reports from `LOCAL_MODEL_RESPONSE_SCHEMA_VERSION`. Copy the value into baselines in `from_report`. Legacy baseline JSON without the field should deserialize with version `0`, meaning unknown legacy contract.

Expose accessors:

- `LocalModelBenchmarkReport::response_schema_version()`
- `LocalModelBenchmarkBaseline::response_schema_version()`

Update the CLI benchmark summary to include `response_schema_version`.

## Non-Goals

- No migration of existing JSONL baseline files.
- No schema compatibility matrix yet.
- No external JSON Schema validation engine.
- No change to proposal scoring semantics.

## Verification

- Benchmark reports expose the current response schema version.
- Baselines created from reports preserve the current response schema version.
- Legacy baseline JSON without the field decodes with version `0`.
- CLI local-model benchmark output reports response schema version `1`.

## Roadmap Impact

This makes local Steward benchmark artifacts version-aware and prepares the response contract for future evolution without weakening baseline reproducibility.
