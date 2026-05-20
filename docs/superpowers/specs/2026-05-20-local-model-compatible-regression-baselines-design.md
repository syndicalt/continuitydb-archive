# Local Model Compatible Regression Baselines Design

## Goal

Compare local-model benchmark regressions only against compatible baseline artifacts.

## Context

Durable local-model baselines now preserve candidate identity, runtime invocation metadata, and response schema version. The regression gate still selects the latest prior baseline by candidate identity alone. That can compare a current run against a baseline produced by a different executable argument set or an older response contract, which makes regression output misleading.

## Architecture

Keep broad candidate lookup available for inspection, but change the record-and-compare gate to compare only with previous baselines that match:

- candidate model ID
- candidate role
- response schema version
- runtime manifest

Add a dedicated compatible-baseline lookup helper and use it from `record_local_model_benchmark_baseline_with_regression`. The current baseline should still be appended even when no compatible previous baseline exists.

## Non-Goals

- No fuzzy runtime matching.
- No schema compatibility matrix.
- No migration of old baseline files.
- No change to pass/fail scoring.

## Verification

- A previous baseline with different runtime metadata does not produce a regression comparison.
- A previous baseline with matching runtime and schema metadata still produces a regression comparison.
- The current baseline is appended in both cases.

## Roadmap Impact

This makes local Steward benchmark gates reproducible and prevents invalid comparisons across model runtime or response contract changes.
