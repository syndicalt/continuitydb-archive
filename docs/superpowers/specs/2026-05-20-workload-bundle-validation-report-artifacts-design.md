# Workload Bundle Validation Report Artifacts Design

## Problem

`validate-workload-bundle` validates archived workload measurement bundles and prints structured JSON, but CI cannot ask it to persist validation evidence. Other artifact-oriented commands already support success and failure report paths. Direct workload bundle validation should have the same durable evidence path.

## Goal

Add:

```sh
continuitydb validate-workload-bundle --artifact-dir <dir> --report-path <path>
continuitydb validate-workload-bundle --artifact-dir <dir> --failure-report-path <path>
```

Successful validations should write the same structured JSON printed to stdout. Failed validations should write a structured failure report before exiting non-zero.

## Design

Extend the `ValidateWorkloadBundle` command with optional report paths. Route validation through a JSON helper so the command can write the success report after validation. On validation error, write a failure report containing:

- `artifact_dir`
- `report_path`
- `failure_report_path`
- `manifest`
- `workload_report`
- `workload_artifacts`
- `failure.stage = "workload_bundle_validation"`
- `failure.message`

For failed validation, fields that cannot be trusted are `null`; this preserves evidence without pretending the archive is valid.

## Non-Goals

- Do not mutate workload bundles.
- Do not replay workloads.
- Do not add new manifest fields.
- Do not change existing validation rules.

## Tests

Add CLI tests that:

- validate a generated workload bundle with `--report-path` and assert the report file matches the printed validation output
- tamper with a workload fixture, run validation with `--failure-report-path`, and assert the failure report records the validation stage and error message
