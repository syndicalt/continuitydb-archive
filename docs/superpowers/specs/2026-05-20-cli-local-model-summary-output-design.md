# CLI Local Model Summary Output Design

## Goal

Expose local model evaluation summary metrics in `continuitydb benchmark-local-model` JSON output.

## Motivation

The library now exposes deterministic `StewardEvaluationSummary` metrics for local model reports and baselines, but the CLI still reconstructs a subset of those metrics manually. Real model benchmark artifacts should be self-describing at the operator boundary and should use the same public summary API as embedders.

## Output Contract

`benchmark-local-model` should include:

- `total_cases`
- `passed_cases`
- `failed_cases`
- `pass_rate`
- `passed`

These fields must be derived from `LocalModelBenchmarkBaseline::evaluation_summary()`.

## Non-Goals

- Do not change benchmark execution.
- Do not change baseline persistence.
- Do not change regression behavior.
- Do not introduce real model downloads or runtime dependencies.
