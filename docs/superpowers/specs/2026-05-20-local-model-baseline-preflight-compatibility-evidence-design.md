# Local Model Baseline Preflight Compatibility Evidence Design

## Problem

`benchmark-local-model --dry-run --compare-baseline` reports whether a compatible local-model benchmark baseline exists, plus its timestamp and contract byte counts. It still omits the matched baseline identity fields used by compatibility filtering: response schema version, evaluation suite fingerprint, schema fingerprint, grammar fingerprint, prompt fingerprint, and runtime manifest. CI and operators therefore cannot audit which persisted compatibility contract was matched without separately parsing the baseline JSONL file.

## Goal

Expose the matched baseline's compatibility identity in dry-run baseline preflight JSON.

## Scope

- Add `previous_response_schema_version` to `baseline_preflight`.
- Add `previous_evaluation_suite_fingerprint`, `previous_schema_fingerprint`, `previous_grammar_fingerprint`, and `previous_prompt_fingerprint` to `baseline_preflight`.
- Add `previous_runtime` with `executable` and `arguments` to `baseline_preflight`.
- Emit `null` for these fields when no compatible baseline exists.
- Keep compatibility matching semantics unchanged.

## Non-Goals

- Do not change baseline comparison or regression logic.
- Do not make byte counts part of compatibility filtering.
- Do not write or mutate baseline files during dry-run preflight.
- Do not add new local-model artifact validation in this slice.

## Verification

- Extend `cli_benchmark_local_model_dry_run_reports_compatible_baseline_preflight` to assert previous compatibility fields match the dry-run current contract and runtime fields.
- Extend `cli_benchmark_local_model_dry_run_compare_reports_missing_baseline_without_creating_file` to assert previous compatibility fields are `null` when no compatible baseline exists.
- Run focused local-model CLI tests and the full workspace verification gate.
