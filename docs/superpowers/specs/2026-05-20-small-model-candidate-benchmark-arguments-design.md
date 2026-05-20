# Small Model Candidate Benchmark Arguments Design

## Problem

Small local Steward candidates now expose recommended runtime metadata, but embedders still need to duplicate how that metadata becomes benchmark runner arguments. This leaves room for inconsistent `llama.cpp` invocations across CLI users, embedding applications, and future baseline collection scripts.

## Goal

Add a deterministic helper that materializes a candidate-recommended local executable runner configuration, and expose the corresponding argument template in `local-model-candidates` JSON.

## Non-Goals

- Do not execute or download models.
- Do not require a grammar file path in candidate registry output.
- Do not make `llama.cpp` the only future runtime forever.
- Do not change benchmark scoring or baseline compatibility.

## Design

Add `SmallModelCandidate::recommended_runner_config(executable, model_path) -> LocalExecutableRunnerConfig`.

For current candidates, the helper should build a conservative `llama.cpp` profile:

- `--model <model-path>`
- `--ctx-size 4096`
- `--temp 0`
- `--prompt -`

The helper intentionally omits `--grammar-file` because the candidate registry does not know where the operator wrote the generated grammar file. The candidate still exposes `requires_grammar = true`, so the omission is explicit and visible in metadata rather than hidden. Operators can add the grammar file when they materialize a concrete benchmark command.

Expose `recommended_runner_arguments` in CLI candidate JSON using placeholder paths:

- executable placeholder: `llama-cli`
- model placeholder: `<model.gguf>`

Only expose the deterministic argument vector, not a shell command string, so callers do not need to parse quoting.

## Acceptance Criteria

- `SmallModelCandidate` can build a `LocalExecutableRunnerConfig` for an executable and model path.
- The default candidate helper returns deterministic `llama.cpp` arguments with model path, context size, temperature, and prompt-stdin marker.
- CLI candidate JSON includes `recommended_runner_arguments`.
- Existing candidate order, candidate roles, and metadata remain unchanged.
