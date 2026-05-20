# Local Model Prompt Fingerprint Design

Persist deterministic prompt-rendering fingerprints on local Steward benchmark reports and baselines.

## Problem

Local model baselines currently preserve runtime arguments, response schema version, evaluation suite fingerprint, schema fingerprint, and grammar fingerprint. The benchmark CLI can also write prompt artifacts, but durable baseline compatibility does not include the prompt rendering contract itself.

If the prompt template changes while evaluation case inputs stay the same, old and new baselines may still compare as compatible. That is wrong for real small-model trials because prompt wording is part of the evaluated contract.

## Goal

Add a `prompt_fingerprint` to:

- `LocalModelBenchmarkReport`
- `LocalModelBenchmarkBaseline`
- `benchmark-local-model` dry-run and run JSON output

The fingerprint must be deterministic for the ordered prompts generated from a `StewardEvaluationSuite` using the same prompt rendering helper as local model execution.

`latest_compatible_local_model_benchmark_baseline` must require matching `prompt_fingerprint` values.

## Boundaries

- Do not change default evaluation cases or scoring.
- Do not record full prompt text in durable baselines.
- Keep legacy baseline decoding backward-compatible by defaulting missing prompt fingerprints to an empty string.
- Do not weaken existing compatibility filters.

## Acceptance Criteria

- Benchmark reports expose a non-empty `fnv1a64:` prompt fingerprint.
- Baselines preserve the report prompt fingerprint.
- Legacy baseline JSON without `prompt_fingerprint` decodes with an empty prompt fingerprint.
- Compatible baseline lookup skips baselines with mismatched prompt fingerprints.
- CLI dry-run and benchmark output include `prompt_fingerprint`.
- README and roadmap document durable prompt fingerprints.
