# Local Model Response Artifacts Design

## Goal

Add `benchmark-local-model --response-dir` so real local Steward model runs can persist raw per-case model responses.

## Context

The CLI can already persist contracts, prompts, benchmark reports, failure reports, and baselines. Real small-model trials still need the raw model stdout for each evaluation case so failures can be diagnosed without rerunning the model.

## Design

Add a feature-gated `--response-dir <path>` option to `benchmark-local-model`. For real runs, the main benchmark evaluation captures the raw response text returned by the model backend for each fixed suite case. The CLI writes one file per case named with the case index and slug, then includes `response_artifacts` in the benchmark JSON.

Each response artifact records:

- `case_name`
- `response_path`
- `response_fingerprint`
- `response_bytes`
- `captured`

If inference fails before producing stdout, the case still appears with `captured = false` and no path or fingerprint. Dry-runs do not execute a model, so they return an empty response artifact list.

## Boundaries

This does not change proposal semantics, policy validation, baseline comparison, or the model-as-proposer rule. It only preserves model stdout for auditability and debugging.

## Documentation

Add README and roadmap entries for CLI local-model raw response artifacts.
