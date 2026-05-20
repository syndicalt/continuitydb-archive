# Local Model Benchmark Dry Run Design

## Problem

`benchmark-local-model` currently runs the configured executable and appends a durable baseline in one step. Operators can inspect candidates, contracts, and the evaluation suite separately, but there is no single preflight artifact showing the exact candidate, runtime arguments, response schema version, and suite fingerprint that a benchmark invocation would use without executing a model or mutating the baseline store.

## Goal

Add `benchmark-local-model --dry-run` to emit a deterministic preflight JSON artifact without invoking the local executable and without appending a baseline record.

## Non-Goals

- Do not change normal benchmark execution behavior.
- Do not validate executable existence in dry-run mode.
- Do not add model downloads or hosted model execution.
- Do not change benchmark scoring, baseline compatibility, or fingerprint algorithms.

## Design

Add a `dry_run` flag to the existing feature-gated `BenchmarkLocalModel` command.

Normal mode keeps the current behavior:

- resolve the fixed candidate;
- build the local executable runner config;
- run the fixed suite;
- append a durable baseline;
- optionally compare regression.

Dry-run mode:

- resolves the fixed candidate, preserving invalid-candidate validation;
- builds the same local executable argument list;
- computes the default suite fingerprint;
- emits JSON with `dry_run: true`, `will_record_baseline: false`, candidate metadata, requested baseline path, response schema version, evaluation suite fingerprint, and runtime manifest;
- does not open the baseline store;
- does not spawn the executable.

## Acceptance Criteria

- `benchmark-local-model --dry-run` succeeds with a nonexistent executable path.
- Dry-run output includes `candidate_model_id`, `candidate_role`, `baseline_path`, `response_schema_version`, `evaluation_suite_fingerprint`, and `runtime`.
- Dry-run output includes `dry_run: true` and `will_record_baseline: false`.
- Dry-run does not create or append the requested baseline file.
- Normal benchmark behavior remains covered by the existing benchmark test.
