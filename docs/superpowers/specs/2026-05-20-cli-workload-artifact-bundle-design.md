# CLI Workload Artifact Bundle Design

## Context

Workload measurements now support direct success reports and regression failure reports, but CI and storage-engine experiments need one self-describing directory that can be archived without coordinating several explicit paths.

## Decision

Add `measure-workload --artifact-dir <dir>`.

The command writes `workload-report.json` with the same JSON emitted to stdout, then writes a versioned `continuitydb-workload.manifest.json` in the same directory. The report includes `artifact_dir` and `bundle_manifest` metadata. The manifest records the report path, kernel, store path, baseline identity, baseline comparison, lookup-plan evidence, and workload summary.

When `--fail-on-regression` rejects a measured run, the command still writes the bundle before returning non-zero. The rejected run is not appended to the baseline store.

## Non-goals

- Do not add per-cell workload fixture dumps in this slice.
- Do not change workload generation, measurement, or regression semantics.
- Do not require `--artifact-dir` when `--report-path` or `--failure-report-path` is enough.
