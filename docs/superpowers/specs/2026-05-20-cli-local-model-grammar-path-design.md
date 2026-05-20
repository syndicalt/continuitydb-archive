# CLI Local Model Grammar Path Design

## Problem

The local Steward model contract command writes a GBNF grammar file, and the candidate registry marks grammar-constrained output as required. `benchmark-local-model` can only pass that grammar file today through generic repeated `--arg` values, which is easy to mistype and hides an important part of the Steward execution contract.

## Goal

Add an explicit `benchmark-local-model --grammar-path <PATH>` option that adds the Steward grammar file to the local runtime manifest.

## Non-Goals

- Do not generate the grammar file automatically.
- Do not require the grammar path for all benchmark runs yet.
- Do not execute a local model during tests.
- Do not add runtime-specific validation beyond deterministic argument construction.

## Design

Add an optional CLI field:

```text
--grammar-path <PATH>
```

When supplied, append the llama.cpp-compatible pair:

```text
--grammar-file <PATH>
```

to the `LocalExecutableRunnerConfig` after the base/candidate-default arguments and before any explicit repeated `--arg` values. This preserves a clear precedence model:

1. Base model path or candidate defaults.
2. First-class grammar path.
3. Operator-supplied extra arguments.

Dry-run JSON already emits the runtime manifest, so tests can verify argument placement without invoking a model.

## Acceptance Criteria

- `benchmark-local-model --dry-run --grammar-path <PATH>` includes `--grammar-file <PATH>` in runtime arguments.
- With `--candidate-defaults`, grammar path appears after candidate default arguments and before explicit `--arg` values.
- Existing benchmark behavior without `--grammar-path` remains unchanged.
- README and roadmap document the new first-class grammar path.
