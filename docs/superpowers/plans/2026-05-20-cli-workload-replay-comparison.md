# CLI Workload Replay Comparison Plan

**Goal:** Let replayed workload bundles prove deterministic count equivalence against the archived measurement report.

**Architecture:** Add `--compare-report` and `--fail-on-mismatch` to `replay-workload`, read `workload-report.json`, compare replayed workload/checkout counts against archived counts, emit stable mismatch JSON, and preserve replay behavior when comparison is not requested.

**Tech Stack:** Rust, clap, serde_json, existing workload CLI tests.

- [x] Add failing CLI coverage for replay report comparison and failure gating.
- [x] Add replay comparison options.
- [x] Compare deterministic workload and checkout counts against `workload-report.json`.
- [x] Emit stable `replay_comparison` JSON.
- [x] Return non-zero when `--fail-on-mismatch` detects mismatch.
- [x] Update README and roadmap.
- [x] Run focused and full verification.
