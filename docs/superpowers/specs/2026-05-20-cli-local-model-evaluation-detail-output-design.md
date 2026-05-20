# CLI Local Model Evaluation Detail Output Design

## Goal

Expose per-case local model evaluation details in `continuitydb benchmark-local-model` JSON output.

## Motivation

The CLI now emits deterministic aggregate summary metrics, but real local model benchmark artifacts also need explainability. Operators should be able to see which fixed evaluation cases ran and which deterministic failure reasons were recorded without opening the baseline JSONL file or re-running the benchmark through library APIs.

## Output Contract

`benchmark-local-model` should include an `evaluation` object containing the serialized `StewardEvaluationReport` from the current baseline:

```json
{
  "evaluation": {
    "case_reports": [
      {
        "name": "insufficient evidence uncertainty",
        "failures": []
      }
    ]
  }
}
```

The existing top-level summary fields remain unchanged.

## Non-Goals

- Do not change the stored baseline format.
- Do not change evaluation pass/fail semantics.
- Do not add a second custom CLI-specific failure schema.
- Do not download or invoke a real model dependency.
