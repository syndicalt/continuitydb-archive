# Local Model Bundle Validation Failure Reports Design

## Problem

`validate-local-model-bundle --artifact-dir` can print successful validation evidence and can now write successful validation reports, but validation failures only emit stderr. That leaves CI without a structured artifact describing why an archived local Steward benchmark bundle was rejected.

## Goal

Add `--failure-report-path` to `validate-local-model-bundle` so validation failures can write structured JSON evidence before exiting non-zero.

## Scope

- Add `--failure-report-path` to the feature-gated command.
- On validation error, write a JSON failure report with artifact path, optional success-report path, failure-report path, and a validation failure object.
- Preserve existing non-zero stderr behavior.
- Do not change successful validation semantics.

## Non-Goals

- Do not add best-effort manifest/report metadata in this slice.
- Do not change validation rules.
- Do not change local-model benchmark bundle generation.

## Proposed Failure Report Shape

```json
{
  "artifact_dir": "...",
  "report_path": null,
  "failure_report_path": "...",
  "manifest": null,
  "benchmark_report": null,
  "changed_case_report": null,
  "response_artifact_manifest": null,
  "failure": {
    "stage": "local_model_bundle_validation",
    "message": "..."
  }
}
```

## Verification

- Add a CLI test that tampers local-model benchmark report byte-count metadata and runs `validate-local-model-bundle --failure-report-path`.
- Assert the command fails, the failure report exists, and it records stage, message, artifact dir, and failure-report path.
- Run focused local-model CLI test and the full workspace gate.
