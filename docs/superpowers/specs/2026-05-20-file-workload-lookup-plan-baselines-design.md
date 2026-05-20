# File Workload Lookup Plan Baselines Design

## Problem

`measure-workload --kernel file` reports lookup-plan diagnostics in live JSON, but baseline JSONL records only persist workload counts and timings. That loses the planner evidence needed to explain future workload regressions or index changes.

## Design

- Add serializable workload lookup-plan snapshot types in `continuitydb-workload`.
- Store lookup-plan snapshots as optional fields on `WorkloadMeasurementSnapshot`.
- Keep the field optional and serde-defaulted so legacy baseline records remain readable.
- Populate the field when the CLI records file-kernel workload baselines.
- Leave memory-kernel baselines without lookup plans because memory kernels do not expose file planner diagnostics.

This stores planner evidence only. It does not add lookup-plan regression gates yet.

## Test

- Assert workload snapshots preserve file lookup-plan labels, per-index candidate counts, final candidate count, and full-scan status.
- Assert `measure-workload --kernel file --baseline-path ...` writes lookup-plan diagnostics into the JSONL baseline record.
