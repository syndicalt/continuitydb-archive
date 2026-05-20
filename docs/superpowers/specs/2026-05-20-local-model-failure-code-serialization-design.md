# Local Model Failure Code Serialization Design

## Problem

`StewardEvaluationFailure` is part of durable benchmark reports and baselines. Its default Rust enum serialization exposes Rust variant casing such as `InvalidModelResponse`, which is less stable for external CI/report consumers than explicit snake-case failure codes.

## Constraints

- New JSON artifacts should use stable snake-case variant names.
- Existing durable benchmark reports and baselines with legacy Rust variant names must remain readable.
- Preserve existing enum structure for payload-carrying failures.
- Do not change deterministic evaluation behavior.

## Test

Add tests proving:

- New failure JSON serializes as `model_execution_failed`, `invalid_model_response`, and snake-case object keys for payload variants.
- Legacy JSON using `ModelExecutionFailed`, `InvalidModelResponse`, and older payload variant names still deserializes.
