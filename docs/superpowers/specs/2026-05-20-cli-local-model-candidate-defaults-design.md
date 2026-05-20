# CLI Local Model Candidate Defaults Design

## Problem

`local-model-candidates` now exposes deterministic benchmark argument templates, but `benchmark-local-model` still requires callers to manually copy those arguments into repeated `--arg` flags. That makes real local baseline collection more error-prone and weakens the candidate registry as an operational contract.

## Goal

Add a `benchmark-local-model --candidate-defaults` option that builds the runner configuration from the selected candidate's recommended benchmark template.

## Non-Goals

- Do not download models.
- Do not execute models during dry-run.
- Do not remove explicit `--arg` support.
- Do not add grammar file path management yet.

## Design

Add a boolean CLI flag:

```text
--candidate-defaults
```

When set, `benchmark-local-model` should call:

```rust
candidate.recommended_runner_config(executable, model_path)
```

instead of starting with only `LocalExecutableRunnerConfig::new(executable).with_model_path(model_path)`.

Any explicit `--arg` values remain repeatable and ordered. They are appended after the candidate defaults so operators can add runtime-specific switches without losing the standard benchmark shape.

Dry-run JSON already exposes the final runtime manifest, so the acceptance test can validate the behavior without executing a local model.

## Acceptance Criteria

- `benchmark-local-model --dry-run --candidate-defaults` includes candidate default arguments in runtime JSON.
- Explicit `--arg` values are appended after candidate defaults.
- Existing behavior without `--candidate-defaults` remains unchanged.
- Roadmap and README document the new CLI surface.
