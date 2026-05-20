# Local Model Changed-Case Report Validation Design

## Context

Local model benchmark bundles can include `changed-cases.json` when `benchmark-local-model --artifact-dir --compare-baseline` is used. The bundle manifest already records `changed_case_report_path` and nested `changed_case_report` metadata with path, fingerprint, and byte count.

`validate-local-model-bundle --artifact-dir` currently validates only the root benchmark report metadata. That leaves changed-case report metadata descriptive rather than enforceable.

## Design

Extend `validate-local-model-bundle --artifact-dir` to validate changed-case report metadata when the manifest contains a changed-case report:

- `manifest["changed_case_report_path"] == <artifact_dir>/changed-cases.json`
- `manifest["changed_case_report"]["report_path"] == <artifact_dir>/changed-cases.json`
- `manifest["changed_case_report"]["report_bytes"] == changed_cases_text.len()`
- `manifest["changed_case_report"]["report_fingerprint"] == fnv1a64_fingerprint(changed_cases_text)`

If no changed-case report is present, validation should continue to succeed and emit `changed_case_report: null`.

On success, include `changed_case_report` in the validation JSON. On failure, return non-zero with specific error categories:

- `local model benchmark manifest changed-case report path mismatch`
- `local model benchmark manifest changed-case report byte count mismatch`
- `local model benchmark manifest changed-case report fingerprint mismatch`

## Testing

Add feature-gated Unix CLI tests that create a real changed-case bundle by recording one passing baseline, changing the runner output, running `benchmark-local-model --compare-baseline --artifact-dir`, then validating:

- untouched changed-case bundle succeeds and reports changed-case metadata
- changed-case report path mismatch fails
- changed-case report byte-count mismatch fails
- changed-case report fingerprint mismatch fails
