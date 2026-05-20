# CLI Workload Report Artifact Design

## Context

`continuitydb measure-workload` prints structured JSON with workload counts, timings, optional file lookup-plan diagnostics, and optional baseline comparison results. CI and operator workflows need an explicit artifact path so the exact successful report can be archived without depending on shell redirection.

Local Steward model benchmarks already support first-class report artifacts. Workload measurement should offer the same operational pattern for storage and planner evidence.

## Decision

Add `measure-workload --report-path <path>`.

When workload measurement succeeds, the CLI writes the same pretty JSON object printed to stdout to the report path. Parent directories are created if needed. The report includes the selected path in `report_path`.

The report is written after successful measurement, comparison, and optional baseline recording. If `--fail-on-regression` causes the command to fail, no successful report artifact is written by this slice.

## Non-goals

- Do not add failure-report artifacts for workload regressions yet.
- Do not change workload measurement semantics.
- Do not change baseline JSONL record format.
