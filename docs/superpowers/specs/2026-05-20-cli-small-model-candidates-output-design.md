# CLI Small Model Candidates Output Design

## Problem

ContinuityDB has a fixed public registry of small local Steward model candidates, but operators can only discover it from Rust APIs or roadmap prose. Before running `benchmark-local-model`, operators need a deterministic CLI artifact that lists the candidate IDs and roles accepted by the benchmark command.

## Goal

Add a feature-gated CLI command that prints the fixed small-model candidate registry as JSON.

## Non-Goals

- Do not change candidate ordering or candidate metadata.
- Do not add download, runtime installation, or hosted model behavior.
- Do not add new candidates in this slice.
- Do not run real model inference.

## Design

Add `continuitydb local-model-candidates` behind the existing `local-model` feature.

The command outputs:

```json
{
  "default_candidate": "Qwen/Qwen2.5-0.5B-Instruct",
  "total_candidates": 4,
  "candidates": [
    {
      "model_id": "Qwen/Qwen2.5-0.5B-Instruct",
      "role": "default-feasibility"
    }
  ]
}
```

The default candidate is the first registry entry, matching existing `benchmark-local-model --candidate` default behavior.

## Acceptance Criteria

- `continuitydb local-model-candidates`, built with the `local-model` feature, exits successfully.
- The JSON includes `default_candidate`, `total_candidates`, and ordered `candidates`.
- The first candidate is `Qwen/Qwen2.5-0.5B-Instruct` with role `default-feasibility`.
- The output includes `HuggingFaceTB/SmolLM2-360M-Instruct` with role `ultra-small-experimental`.
- README and roadmap document the new operator artifact.
