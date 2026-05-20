# Public Compatible Local Model Baseline Lookup Design

## Goal

Expose the local-model baseline compatibility rule as a public embeddable API so callers can inspect the newest valid comparison baseline without recording a new benchmark run.

## Motivation

`record_local_model_benchmark_baseline_with_regression` already compares a current local-model benchmark only with previous baselines that match candidate identity, response schema version, and runtime manifest. Embedders need the same compatibility semantics for dashboards, CI preflight checks, and benchmark inspection flows that do not append a new baseline.

## API

Add a feature-gated public function in `continuitydb-steward`:

```rust
pub fn latest_compatible_local_model_benchmark_baseline<S>(
    store: &S,
    current: &LocalModelBenchmarkBaseline,
) -> Result<Option<LocalModelBenchmarkBaseline>, StewardError>
where
    S: LocalModelBenchmarkBaselineStore;
```

Compatibility remains strict:

- same candidate model ID
- same candidate role
- same local-model response schema version
- same runtime manifest

The function returns the newest compatible baseline by `recorded_at`, or `None` when no stored baseline can be compared to the current baseline.

## Non-Goals

- Do not change regression scoring.
- Do not append baselines from this lookup.
- Do not broaden compatibility with fuzzy runtime matching.
- Do not add model-specific benchmark artifacts.
