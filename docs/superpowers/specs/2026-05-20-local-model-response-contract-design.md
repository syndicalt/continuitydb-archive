# Local Model Response Contract Design

## Context

ContinuityDB already has a feature-gated `continuitydb-steward` local model boundary, executable runner profiles, a deterministic decoder, and a fixed proposal-quality evaluation harness. The remaining runtime gap is that external model runners need a stable output contract they can constrain against before the crate attempts to decode proposals.

The current decoder expects a JSON object with a `proposals` array. Each proposal contains an `action`, `rationale`, and `citations`. The supported action variants mirror `StewardAction`: create cell draft, link revision, adjust confidence, label answerability, mark frontier, and request verification.

## Design

Add a first-class local model response contract to the feature-gated Steward module:

- `LOCAL_MODEL_RESPONSE_SCHEMA_VERSION`: a stable version integer for the model-output contract.
- `local_model_response_json_schema() -> &'static str`: a JSON Schema string embedders can write to disk or pass to runtimes that support schema-constrained output.
- `local_model_response_gbnf_grammar() -> &'static str`: a conservative GBNF grammar string for llama.cpp-style constrained JSON generation.

The schema is the authoritative contract for fields and action variants. The grammar is intentionally structural: it constrains the response to a JSON object with `proposals`, proposal objects, action objects, rationales, and citations, while leaving UUID and text validation to the existing deterministic decoder and policy layer. This keeps model-facing constraints practical without duplicating every Rust validation rule in grammar text.

## Runtime Profile Integration

`LlamaCppRuntimeProfile` already accepts a grammar file path. This slice should not write files automatically or introduce model downloads. Instead it should expose contract text and add convenience constructors that bind existing runtime profile APIs to user-provided contract files:

- `LlamaCppRuntimeProfile::with_steward_response_grammar_file(path)` delegates to `with_grammar_file`.
- `MistralRsRuntimeProfile::with_steward_json_output()` delegates to `with_json_output`.

These names make caller intent explicit while keeping existing runner arguments stable.

## Testing

Tests should prove:

- the JSON Schema string is valid JSON and includes the expected top-level version, `proposals` array, and action discriminator;
- the GBNF grammar includes the required root and proposal/action productions used by llama.cpp-style runners;
- the existing `LlamaCppRuntimeProfile` and `MistralRsRuntimeProfile` command arguments include the selected contract-related flags through the new convenience methods.

## Roadmap Impact

This adds a Steward milestone for a published local model response contract. It moves the real-runtime path forward by giving embedders and benchmark scripts a stable schema/grammar target while preserving the rule that model output remains a proposal decoded and validated by deterministic Rust policy.
