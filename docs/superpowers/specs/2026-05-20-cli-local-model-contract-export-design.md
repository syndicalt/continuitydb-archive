# CLI Local Model Contract Export Design

## Goal

Expose the local Steward model response contract as operator-visible CLI artifacts.

## Context

The Steward local-model crate already publishes a JSON Schema and GBNF grammar through Rust accessors. The CLI can now run a configured local model benchmark, but real llama.cpp or wrapper-based experiments still need contract files on disk so the runtime can constrain output and CI can archive the exact contract used with a baseline.

## Architecture

Add a feature-gated `continuitydb local-model-contract` command that writes:

- the JSON Schema from `local_model_response_json_schema()`
- the GBNF grammar from `local_model_response_gbnf_grammar()`

The command accepts explicit output paths and prints structured JSON including schema version and output paths. It does not run a model or mutate database state.

## Non-Goals

- No schema generation from runtime state.
- No hosted model integration.
- No validation against an external JSON Schema engine.
- No automatic mutation of benchmark runner arguments.

## Verification

- CLI smoke test runs with `--features local-model`.
- The command writes both files.
- The schema file decodes as JSON and includes the stable schema ID/version.
- The grammar file includes the expected root production and request-verification action.
- Output JSON reports both paths and schema version.

## Roadmap Impact

This makes grammar-constrained local Steward model evaluation reproducible from the CLI, closing a practical gap between the response contract and real executable benchmark runs.
