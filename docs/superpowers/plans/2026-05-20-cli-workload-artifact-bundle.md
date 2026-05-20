# CLI Workload Artifact Bundle Plan

**Goal:** Give workload measurement and regression gates a single archiveable artifact directory for CI and storage-engine benchmarking.

**Architecture:** Add `--artifact-dir` to `measure-workload`, include bundle metadata in workload JSON, write `workload-report.json`, write a versioned manifest, and perform the same bundle write before non-zero regression exits.

**Tech Stack:** Rust, clap, serde_json, existing workload CLI tests.

- [x] Add failing CLI coverage for successful `measure-workload --artifact-dir`.
- [x] Add failing CLI coverage for regression-gated `measure-workload --artifact-dir`.
- [x] Add CLI option and option plumbing.
- [x] Write workload report and manifest bundle.
- [x] Preserve failed-regression no-baseline-recording behavior.
- [x] Update README and roadmap.
- [x] Run focused and full verification.
