# Local Model Evaluation Summary Design

## Goal

Add a first-class deterministic summary for local Steward model evaluation reports and benchmark baselines.

## Motivation

Local model benchmarks currently expose raw case reports and a boolean `passed()` result. Regression gates internally count passed cases, but embedders cannot inspect total cases, passed cases, failed cases, or pass rate without duplicating crate-private logic. The embedded database Steward roadmap needs benchmark artifacts that are easy to compare, display, and audit before real local model artifacts are collected.

## API

Add a serializable summary type:

```rust
pub struct StewardEvaluationSummary {
    total_cases: usize,
    passed_cases: usize,
    failed_cases: usize,
}
```

Expose accessors:

- `total_cases() -> usize`
- `passed_cases() -> usize`
- `failed_cases() -> usize`
- `pass_rate() -> f64`
- `passed() -> bool`

Add summary accessors:

- `StewardEvaluationReport::summary() -> StewardEvaluationSummary`
- `LocalModelBenchmarkReport::evaluation_summary() -> StewardEvaluationSummary`
- `LocalModelBenchmarkBaseline::evaluation_summary() -> StewardEvaluationSummary`

Regression comparison should use the public summary API instead of private pass-count logic.

## Semantics

- `total_cases` is the number of evaluated cases.
- `passed_cases` counts case reports with no failures.
- `failed_cases` is `total_cases - passed_cases`.
- `pass_rate` is `1.0` for an empty suite so empty smoke fixtures remain neutral instead of creating a divide-by-zero special case for embedders.
- `passed()` is true when `failed_cases == 0`.

## Non-Goals

- Do not change evaluation pass/fail rules.
- Do not add weighted scoring.
- Do not change persisted baseline compatibility.
- Do not add real model artifacts.
