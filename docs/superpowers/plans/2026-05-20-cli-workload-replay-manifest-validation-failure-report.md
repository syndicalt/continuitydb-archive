# CLI Workload Replay Manifest Validation Failure Report Plan

## Goal

Add structured failure report artifacts for workload replay input-manifest validation failures.

## Steps

- [x] Add a failing CLI test for `replay-workload --require-manifest --failure-report-path` on a tampered fixture.
- [x] Write validation-failure JSON before returning the validation error.
- [x] Include failure stage, message, artifact paths, fixture fingerprints, and byte counts.
- [x] Update README and roadmap milestone tracking.
- [x] Run focused and workspace verification gates.
- [x] Commit the completed slice.

## Verification

```sh
cargo test -p continuitydb-cli cli_replay_workload_require_manifest_failure_report_records_validation_failure
cargo test -p continuitydb-cli cli_replay_workload_require_manifest_rejects_tampered_fixture
cargo test -p continuitydb-cli cli_replay_workload_require_manifest_reports_validated_manifest
cargo fmt --all -- --check
git diff --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
```
