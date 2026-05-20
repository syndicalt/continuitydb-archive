# CLI Workload Report Artifact Plan

**Goal:** Add a first-class workload measurement report artifact path so CI and operator workflows can archive the exact JSON emitted by `measure-workload`.

**Architecture:** Extend the `MeasureWorkload` CLI command with `--report-path`, pass it through `WorkloadMeasureOptions`, include it in the JSON payload, and write the same pretty JSON object to disk using the existing JSON-file writer helper.

**Tech Stack:** Rust, clap, serde_json, existing CLI integration tests.

- [x] Add failing CLI coverage for `measure-workload --report-path`.
- [x] Add the CLI option and option plumbing.
- [x] Write successful measurement JSON to the requested artifact path.
- [x] Include `report_path` in the measurement JSON.
- [x] Update README and roadmap.
- [x] Run focused and full verification.
