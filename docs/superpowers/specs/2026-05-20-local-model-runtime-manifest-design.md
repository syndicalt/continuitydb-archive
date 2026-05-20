# Local Model Runtime Manifest Design

## Goal

Persist reproducible local-model runtime metadata with every Steward benchmark report and durable baseline.

## Context

ContinuityDB has a feature-gated local model Steward boundary, deterministic runtime profiles, executable runner benchmarks, and durable JSONL benchmark baselines. The baseline currently records candidate identity, candidate role, evaluation result, and timestamp, but it does not preserve the exact executable invocation that produced the result.

That is too weak for real small-model evaluation artifacts. A stored baseline should explain which local executable and deterministic arguments were used so operators can reproduce or compare Qwen, SmolLM, llama.cpp, and mistral.rs runs without relying on external notes.

## Architecture

Add `LocalModelRuntimeManifest`:

- `executable: String`
- `arguments: Vec<String>`

Populate it from `LocalExecutableRunnerConfig` when `LocalModelBenchmark::run` builds a `LocalModelBenchmarkReport`.

Store the same runtime manifest in `LocalModelBenchmarkBaseline`.

Compatibility:

- Existing JSONL baselines without a runtime manifest should still deserialize.
- Missing runtime manifests deserialize to an empty manifest.

## Non-Goals

- This does not store environment variables.
- This does not hash model files.
- This does not download or run a real model.
- This does not change pass/fail scoring semantics.

## Verification

- Benchmark reports expose executable and full deterministic argument list.
- Baselines created from reports preserve runtime manifests.
- Legacy baseline JSON without runtime metadata still decodes with an empty runtime manifest.

## Roadmap Impact

- Steward local-model track: durable benchmark baselines now carry reproducible runtime invocation metadata, moving environment-specific model evaluation artifacts closer to production usefulness.
