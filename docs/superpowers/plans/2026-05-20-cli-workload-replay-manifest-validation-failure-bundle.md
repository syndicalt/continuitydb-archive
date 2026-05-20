# CLI Workload Replay Manifest Validation Failure Bundle Plan

## Goal

Write replay artifact bundles for workload replay input-manifest validation failures.

## Steps

- [x] Add a failing CLI test for `replay-workload --require-manifest --replay-artifact-dir` on a tampered fixture.
- [x] Reuse the validation failure report payload for replay artifact bundle output.
- [x] Include failure metadata in the replay bundle manifest.
- [x] Update README and roadmap milestone tracking.
- [x] Run focused and workspace verification gates.
- [x] Commit the completed slice.

## Verification

```sh
cargo test -p continuitydb-cli cli_replay_workload_require_manifest_writes_validation_failure_bundle
cargo test -p continuitydb-cli cli_replay_workload_require_manifest_failure_report_records_validation_failure
cargo test -p continuitydb-cli cli_replay_workload_require_manifest_reports_validated_manifest
cargo fmt --all -- --check
git diff --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
```
