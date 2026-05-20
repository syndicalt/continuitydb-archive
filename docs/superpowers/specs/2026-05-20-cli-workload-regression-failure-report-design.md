# CLI Workload Regression Failure Report Design

## Context

`measure-workload --fail-on-regression` can reject a current workload run before recording a new baseline. The command now supports successful report artifacts, but regression gate failures still need structured JSON evidence for CI artifacts and debugging storage or planner changes.

## Decision

Add `measure-workload --failure-report-path <path>`.

When baseline comparison detects a regression and `--fail-on-regression` is set, the CLI writes the current workload measurement JSON to the failure report path before returning the non-zero regression error. The report includes `baseline_comparison`, `failure_report_path`, and the same workload, checkout, timing, and lookup-plan fields as normal output.

The failed run is not appended to the baseline store.

## Non-goals

- Do not write failure reports for measurement errors before a workload report exists.
- Do not add artifact bundles for workload measurements in this slice.
- Do not change baseline comparison semantics.
