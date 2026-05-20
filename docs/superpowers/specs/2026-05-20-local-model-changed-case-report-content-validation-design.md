# Local Model Changed-Case Report Content Validation Design

## Problem

`validate-local-model-bundle --artifact-dir` validates that an archived local-model `changed-cases.json` file matches the root bundle manifest's path, byte count, and fingerprint metadata. That proves the changed-case report file has not drifted from the manifest, but it does not prove the report still agrees with the archived `benchmark-report.json` that generated it.

For Steward model trials, changed-case reports are compact routing artifacts for CI dashboards and review workflows. If a bundle contains a valid changed-case report file whose `candidate_model_id`, `baseline_path`, or comparison summary no longer matches the benchmark report, the bundle is internally inconsistent even when every file fingerprint matches its manifest entry.

## Goal

Extend local-model bundle validation so changed-case report content is checked against the archived benchmark report content.

## Non-Goals

- Do not change benchmark report generation.
- Do not change changed-case report schema.
- Do not validate raw local-model response artifacts in this slice.
- Do not add real model execution or external model dependencies.

## Design

`validate_local_model_bundle_manifest` already parses `benchmark-report.json` before delegating to changed-case report validation. Pass that parsed report into `validate_local_model_changed_case_report_manifest`.

After path, byte-count, and fingerprint checks pass, parse `changed-cases.json` and validate:

- `format == "continuitydb.local_model.changed_cases"`.
- `format_version == 1`.
- `candidate_model_id` matches `benchmark_report["candidate_model_id"]`.
- `baseline_path` matches `benchmark_report["baseline_path"]`.
- `changed_case_report_path` matches the canonical archived path.
- `comparison` matches the compact comparison projection generated from `benchmark_report["baseline_comparison"]`.

Use the same projection fields as `write_local_model_changed_case_report`:

- `compared`
- `regressed`
- `previous_recorded_at`
- `current_recorded_at`
- `changed_cases`
- `outcome_changed_cases`
- `failure_count_changed_cases`
- `response_changed_cases`
- `regressed_cases`
- `recovered_cases`
- `changed_case_summaries`

## Error Boundary

Use one explicit error for content mismatch:

`local model changed-case report content mismatch`

Keep existing metadata errors unchanged so operators can distinguish path/fingerprint corruption from internally inconsistent report content.

## Tests

Add feature-gated Unix CLI tests that create a real changed-case artifact bundle, mutate `changed-cases.json`, refresh the root manifest's changed-case report byte count and fingerprint, then assert `validate-local-model-bundle` rejects the bundle with the content-mismatch error.

Test cases:

- Candidate model ID mismatch.
- Comparison summary mismatch.

These two cases prove both top-level changed-case report identity and nested comparison content are validated.
