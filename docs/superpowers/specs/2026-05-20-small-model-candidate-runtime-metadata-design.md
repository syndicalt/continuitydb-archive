# Small Model Candidate Runtime Metadata Design

## Problem

`continuitydb local-model-candidates` exposes fixed local Steward candidate IDs and roles, but it does not expose enough machine-readable guidance for an operator or embedding application to select a runtime path. The roadmap names `llama.cpp`/GGUF as the first portability target and `mistral.rs` as the Rust-native path, but candidate metadata does not carry those recommendations.

## Goal

Add first-class runtime metadata to each `SmallModelCandidate` and expose it through the CLI candidate registry JSON.

## Non-Goals

- Do not download models.
- Do not add a new model runtime dependency.
- Do not claim a specific quantized artifact filename.
- Do not change benchmark scoring or execution.

## Design

Extend `SmallModelCandidate` with conservative static metadata:

- `recommended_runtime`
- `artifact_format`
- `recommended_temperature`
- `requires_grammar`
- `notes`

Use `llama.cpp` and `GGUF` for the initial candidate registry because the roadmap's first recommended inference path is portable grammar-constrained local execution. Set `recommended_temperature` to `0.0` because benchmark acceptance requires stable low-temperature output. Set `requires_grammar` to `true` because the Steward response contract is schema/grammar constrained and model output must remain proposal-only input to deterministic policy.

Expose public accessors so embedders can use the metadata without parsing CLI JSON. Update CLI candidate output to include the same metadata for every candidate.

## Acceptance Criteria

- `SmallModelCandidate` exposes runtime metadata through public accessors.
- Every fixed candidate has non-empty runtime and artifact metadata.
- Every fixed candidate marks grammar-constrained output as required.
- CLI candidate registry JSON includes the metadata for every candidate.
- Existing candidate ordering and roles remain unchanged.
