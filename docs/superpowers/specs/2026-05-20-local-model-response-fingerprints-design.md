# Local Model Response Fingerprints Design

## Goal

Persist deterministic per-case local Steward model response fingerprints in benchmark reports and durable baselines.

## Context

`benchmark-local-model --response-dir` can now write raw per-case model stdout artifacts, but the durable baseline itself does not preserve enough output identity to audit which raw responses produced the recorded evaluation. Baselines should remain lightweight and should not embed raw model text, but they should carry deterministic response metadata.

## Design

Add a serializable per-case response fingerprint summary to `LocalModelBenchmarkReport` and `LocalModelBenchmarkBaseline`:

- `case_name`
- `captured`
- `response_fingerprint`
- `response_bytes`

The fingerprint is computed over the exact raw backend response text using the existing FNV-1a field-fingerprint convention. If inference fails before stdout is returned, `captured = false`, fingerprint is absent, and byte count is zero. If stdout is returned but proposal decoding fails, the raw response is still fingerprinted.

Benchmark evaluation should capture raw responses once, decode from the captured response, and store only fingerprint summaries in the report/baseline. CLI JSON should expose the durable response fingerprint summaries alongside optional response artifact file metadata.

## Boundaries

This does not store raw model responses in baselines, change proposal scoring, change regression compatibility, or make response fingerprints part of compatible-baseline matching. Response fingerprints are audit metadata, not a quality gate.

## Documentation

Add README and roadmap entries for durable local-model response fingerprints.
