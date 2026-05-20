# CLI Workload Regression Failure Report Plan

**Goal:** Preserve structured workload regression evidence when `measure-workload --fail-on-regression` exits non-zero.

**Architecture:** Add a `--failure-report-path` CLI option to `measure-workload`, include it in `WorkloadMeasureOptions` and output JSON, assemble the workload report before the regression gate returns, and write the pretty JSON report when the gate fails.

**Tech Stack:** Rust, clap, serde_json, existing workload CLI tests.

- [x] Add failing CLI coverage for `measure-workload --fail-on-regression --failure-report-path`.
- [x] Add CLI option and option plumbing.
- [x] Assemble workload JSON before the regression gate.
- [x] Write failure report JSON before returning the non-zero gate error.
- [x] Verify the failed run is not appended as a new baseline.
- [x] Update README and roadmap.
- [x] Run focused and full verification.
