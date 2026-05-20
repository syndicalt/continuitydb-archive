# File Workload Lookup Plan Regression Plan

**Goal:** Make persisted file workload lookup-plan baselines actionable by detecting deterministic lookup-plan regressions during workload baseline comparison.

**Architecture:** Extend `WorkloadBaselineRegression` and `compare_workload_snapshot_to_baseline` in `continuitydb-workload`. The CLI already serializes comparison regressions through the shared workload API, so no separate CLI comparison path is needed.

**Tech Stack:** Rust, serde-compatible regression enum variants, existing workload baseline comparison tests.

- [x] Add failing tests for lookup-plan presence changes, missing-plan compatibility, and plan-shape changes.
- [x] Add lookup-plan regression variants.
- [x] Compare optional lookup plans from baseline and current snapshots.
- [x] Compare per-constraint candidate counts for shared indexed constraint names.
- [x] Update README and roadmap.
- [x] Run focused and full verification.
