# Local Model Evaluation Suite Fingerprints Design

## Problem

Local model benchmark baselines preserve candidate identity, response schema version, and runtime manifest. They do not preserve which evaluation suite produced the score. After adding visible suite contracts and multiple fixed cases, this creates a compatibility gap: if the default suite changes, a new benchmark can be compared against an older baseline that used a different case contract.

## Goal

Add a deterministic evaluation suite fingerprint to local model benchmark reports and durable baselines, then include that fingerprint in compatible-baseline lookup and CLI benchmark JSON.

## Design

- Add `StewardEvaluationSuite::fingerprint(&self) -> String`.
- Compute the fingerprint from the ordered case contract: case name, input timestamp, task, evidence locators/text, expected actions, required citations, required rationale terms, and forbidden rationale terms.
- Store the fingerprint on `LocalModelBenchmarkReport` and `LocalModelBenchmarkBaseline`.
- Decode legacy baselines with an empty fingerprint so old records remain readable.
- Require fingerprint equality in `latest_compatible_local_model_benchmark_baseline`.
- Emit `evaluation_suite_fingerprint` in `benchmark-local-model` JSON.

The initial hash uses a small deterministic FNV-1a 64-bit implementation over a canonical field stream. This avoids pulling in a new hash dependency while still making contract drift visible and filterable.

## Non-Goals

- Do not make the fingerprint cryptographic.
- Do not change scoring logic.
- Do not migrate legacy baseline files.
- Do not change the response schema contract for model output.

## Acceptance Criteria

- Tests prove suite fingerprints are deterministic and change when the case contract changes.
- Benchmark reports and durable baselines expose the suite fingerprint.
- Legacy baseline JSON decodes with an empty suite fingerprint.
- Compatible-baseline lookup ignores newer baselines with a different suite fingerprint.
- CLI benchmark JSON includes `evaluation_suite_fingerprint`.
