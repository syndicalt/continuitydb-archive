# CLI Local Model Evaluation Suite Export Design

## Problem

The Steward library exposes read-only local-model evaluation contracts, but CLI users still need to run a benchmark to see which fixed cases a candidate is being judged against. That weakens benchmark transparency for operators who want to audit case tasks, evidence, expected actions, and rationale constraints before spending model runtime.

## Goal

Add a feature-gated CLI inspection command that prints the default local-model Steward evaluation suite as deterministic JSON.

## Command

`continuitydb local-model-evaluation-suite`

The command takes no arguments in this slice. It exports the built-in default suite used by `benchmark-local-model`.

## JSON Shape

```json
{
  "response_schema_version": 1,
  "total_cases": 2,
  "cases": [
    {
      "name": "insufficient evidence uncertainty",
      "created_at": "2026-05-20T00:00:00Z",
      "task": "Assess whether thin evidence needs verification.",
      "evidence": [
        {
          "locator": "continuitydb://evaluation/thin-evidence",
          "text": "One weak source mentions the claim without corroboration."
        }
      ],
      "expected_actions": [
        {
          "type": "request_verification",
          "cell_id": null,
          "request": "Gather additional source evidence."
        }
      ],
      "required_citations": ["continuitydb://evaluation/thin-evidence"],
      "required_rationale_terms": ["uncertainty"],
      "forbidden_rationale_terms": []
    }
  ]
}
```

## Constraints

- Use the public suite, case, input, and evidence accessors.
- Do not make `StewardEvaluationCase` serializable in this slice.
- Do not duplicate the benchmark suite definition in CLI code.
- Format `expected_actions` with the stable local-model response action shape rather than Rust enum serialization.
- Do not change `benchmark-local-model`, scoring, baselines, or local model runtime behavior.

## Acceptance Criteria

- A feature-gated CLI test proves `local-model-evaluation-suite` prints both default cases.
- The test verifies case names, tasks, evidence locators, expected action types, required citations, required rationale terms, forbidden rationale terms, and response schema version.
- README and roadmap include the new CLI evaluation suite export.
- Full workspace verification passes.
