# CLI Local Model Candidate Requirements Design

## Problem

Small Steward model candidates declare whether grammar-constrained output is required. The CLI can now pass a grammar path, but benchmark runs can still accidentally omit it and produce unconstrained local model baselines. That weakens the benchmark artifact as evidence for a proposal-only database Steward.

## Goal

Add an opt-in strict CLI gate that enforces selected candidate runtime requirements before benchmark execution or dry-run reporting.

## Non-Goals

- Do not make grammar path globally mandatory yet.
- Do not inspect grammar file contents.
- Do not change local model scoring.
- Do not change existing behavior unless the strict flag is supplied.

## Design

Add:

```text
benchmark-local-model --enforce-candidate-requirements
```

When set, the CLI checks the selected `SmallModelCandidate` before building the benchmark:

- If `candidate.requires_grammar()` is true and `--grammar-path` is absent, return an error.
- If `--grammar-path` is present, proceed normally.

This check applies to both dry-runs and real benchmark runs. Dry-run remains non-mutating, but strict dry-run can fail fast when the configured run would violate candidate requirements.

## Acceptance Criteria

- `benchmark-local-model --dry-run --enforce-candidate-requirements` fails for a grammar-required candidate without `--grammar-path`.
- The error message mentions the missing `--grammar-path`.
- Existing benchmark dry-runs without the strict flag keep working.
- Strict dry-runs with `--grammar-path` keep working.
- README and roadmap document candidate requirement enforcement.
