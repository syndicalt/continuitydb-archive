# CLI Local Model Benchmark Design

## Goal

Expose a feature-gated CLI path that runs the fixed local Steward evaluation suite against a configured local executable and records a durable benchmark baseline.

## Context

ContinuityDB already has the local-model Steward boundary, runtime profiles, benchmark reports, runtime manifests, regression comparison, and JSONL baseline storage in `continuitydb-steward`. That is not yet enough for real model evaluation because operators still need to write Rust code to record a baseline for `llama.cpp`, `mistral.rs`, Qwen, or SmolLM candidates.

The next production step is an operator-facing command that turns the library harness into a reproducible baseline artifact.

## Architecture

- Add a public `default_steward_evaluation_suite()` in `continuitydb-steward`.
- Add an optional `local-model` feature to `continuitydb-cli` that enables `continuitydb-steward/local-model`.
- Add `continuitydb benchmark-local-model` behind that feature.
- The command accepts candidate model ID, executable path, model path, repeated runtime arguments, baseline path, and optional regression flags.
- The command records a JSONL baseline through `FileLocalModelBenchmarkBaselineStore`.
- The command prints JSON summary output with candidate identity, runtime manifest, case counts, pass status, baseline path, and regression status.

## Non-Goals

- No model downloads.
- No bundled GGUF files.
- No hosted model APIs.
- No mutation of committed StateCells from model output.
- No broad CLI evaluation-suite authoring language yet.

## Verification

- A CLI smoke test uses a temporary executable script that emits valid Steward JSON.
- The command writes one JSONL baseline record.
- Output JSON includes candidate metadata, runtime manifest, pass count, and baseline path.
- The stored baseline includes the same runtime manifest.
- Default CLI builds continue to work without the local-model feature.

## Roadmap Impact

This turns the small embeddable model track into an executable baseline workflow while preserving the model-as-proposer boundary.
