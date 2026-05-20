# CLI Workload Replay Manifest Report Content Validation Plan

## Goal

Validate archived workload report content against required workload replay manifests.

## Steps

- [x] Add a failing CLI test for a tampered `workload-report.json` whose manifest remains intact.
- [x] Validate manifest-owned workload report fields against the archived report.
- [x] Update README and roadmap milestone tracking.
- [x] Run focused and workspace verification gates.
- [x] Commit the completed slice.

## Verification

```sh
cargo test -p continuitydb-cli cli_replay_workload_require_manifest_rejects_report_content_mismatch
cargo test -p continuitydb-cli cli_replay_workload_require_manifest_rejects_fixture_byte_count_mismatch
cargo test -p continuitydb-cli cli_replay_workload_require_manifest_reports_validated_manifest
cargo fmt --all -- --check
git diff --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
```
