# CLI Workload Replay Bundle Artifacts Design

## Problem

`replay-workload` can write a standalone replay report, but CI artifact collectors benefit from a single replay output directory with a stable manifest. The original workload bundle remains the input fixture; replay evidence should have its own archiveable bundle.

## Design

- Add `replay-workload --replay-artifact-dir <dir>`.
- Write `<dir>/replay-report.json` for successful replay runs.
- Write `<dir>/replay-report.json` before returning a `--fail-on-mismatch` error when replay comparison fails.
- Write `<dir>/continuitydb-workload-replay.manifest.json` with format metadata, replay report path, input artifact directory, kernel, store path, workload artifact metadata, lookup plan, and replay comparison.
- Add `replay_artifact_dir` and `replay_bundle_manifest` fields to replay JSON.
- Keep `--report-path` and `--failure-report-path` behavior unchanged; explicit report paths remain single-file artifacts.

## Test

- Add a CLI test that creates a workload artifact bundle, mutates its archived `workload-report.json`, runs `replay-workload --fail-on-mismatch --replay-artifact-dir`, asserts the command fails, and verifies the replay report and manifest are written with mismatch evidence.
