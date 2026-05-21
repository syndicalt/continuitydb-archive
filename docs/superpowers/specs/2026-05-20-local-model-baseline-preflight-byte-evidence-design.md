# Local Model Baseline Preflight Byte Evidence Design

## Problem

`benchmark-local-model --dry-run --compare-baseline` can match durable local-model benchmark baselines that persist response schema and grammar byte counts, but the preflight JSON only reports whether a compatible baseline exists and when it was recorded. Operators cannot see the matched baseline contract byte evidence from the dry-run compatibility report.

## Goal

Expose the compatible baseline's persisted response schema and GBNF grammar byte counts in dry-run baseline preflight JSON.

## Scope

- Add `previous_schema_bytes` and `previous_grammar_bytes` to `baseline_preflight`.
- Populate those fields when a compatible baseline is found.
- Emit `null` for those fields when no compatible baseline exists.
- Keep compatibility matching based on existing identity fields and fingerprints.

## Non-Goals

- Do not make baseline compatibility depend on byte counts.
- Do not change real benchmark execution behavior.
- Do not write or mutate baseline files during dry-run preflight.
- Do not add artifact validation in this slice.

## Verification

- Extend `cli_benchmark_local_model_dry_run_reports_compatible_baseline_preflight` to assert the matched preflight byte counts are positive and equal to the dry-run contract byte counts.
- Extend `cli_benchmark_local_model_dry_run_compare_reports_missing_baseline_without_creating_file` to assert missing-compatible-baseline byte evidence is `null`.
- Run the focused local-model CLI tests and the full workspace verification gate.
