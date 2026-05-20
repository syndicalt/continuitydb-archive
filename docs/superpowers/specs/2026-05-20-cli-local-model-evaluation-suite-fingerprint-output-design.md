# CLI Local Model Evaluation Suite Fingerprint Output Design

## Problem

`benchmark-local-model` records and reports the deterministic local-model evaluation suite fingerprint, but `local-model-evaluation-suite` only exports the case contracts. Operators cannot inspect the suite contract and directly compare that inspection artifact with benchmark baselines.

## Goal

Expose the deterministic evaluation suite fingerprint in the feature-gated `local-model-evaluation-suite` JSON output.

## Non-Goals

- Do not change the fingerprint algorithm.
- Do not change the benchmark baseline format.
- Do not add new evaluation cases.
- Do not run real local model inference.

## Design

Add a top-level `evaluation_suite_fingerprint` string to `local_model_evaluation_suite_json()`, populated from `default_steward_evaluation_suite().fingerprint()`.

The field name intentionally matches `benchmark-local-model` output and durable baseline records so operator artifacts can be compared without translation.

## Acceptance Criteria

- `continuitydb local-model-evaluation-suite`, built with the `local-model` feature, outputs a top-level `evaluation_suite_fingerprint`.
- The value uses the existing deterministic fingerprint format.
- Existing case contract output remains unchanged.
- README and roadmap document the new inspection artifact.
