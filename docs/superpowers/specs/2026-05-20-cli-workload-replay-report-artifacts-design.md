# CLI Workload Replay Report Artifacts Design

## Problem

`replay-workload` can reproduce archived workload bundles and compare deterministic counts against `workload-report.json`, but its replay evidence only appears on stdout. CI systems need durable replay artifacts, especially when `--fail-on-mismatch` exits non-zero.

## Design

- Add `replay-workload --report-path <path>` to write the replay JSON report for successful replay runs.
- Add `replay-workload --failure-report-path <path>` to write the replay JSON report before returning a mismatch failure.
- Include `report_path` and `failure_report_path` fields in replay JSON so archived reports are self-describing.
- Keep `--fail-on-mismatch` as implying report comparison through the existing command normalization.

## Test

- Add a CLI test that creates a workload artifact bundle, mutates its archived report to create a deterministic mismatch, runs `replay-workload --fail-on-mismatch --failure-report-path`, asserts the command fails, and verifies the failure report file contains the replay comparison mismatch.
