# CLI Workload Replay Artifacts Plan

**Goal:** Make workload artifact bundles replayable by preserving the exact generated StateCells and checkout request.

**Architecture:** Serialize deterministic workload cells and the measured checkout request into versioned JSON artifacts under `--artifact-dir`, expose fingerprints in the report, and include the same metadata in the bundle manifest for successful and regression-gated runs.

**Tech Stack:** Rust, serde_json, existing workload CLI tests.

- [x] Add failing CLI coverage for workload cells and checkout request artifacts.
- [x] Write `workload-cells.json` with deterministic generated StateCells.
- [x] Write `checkout-request.json` with the measured checkout predicate.
- [x] Include replay artifact metadata in reports and manifests.
- [x] Preserve regression-gated bundle output without recording rejected baselines.
- [x] Update README and roadmap.
- [x] Run focused and full verification.
