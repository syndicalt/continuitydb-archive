# CLI Workload Replay Manifest Report Path Validation Plan

## Goal

Validate the archived workload report path inside required workload replay manifests.

## Steps

- [x] Add a failing CLI test for a manifest whose `workload_report_path` no longer matches `<artifact-dir>/workload-report.json`.
- [x] Validate manifest `workload_report_path`.
- [x] Update README and roadmap milestone tracking.
- [x] Run focused and workspace verification gates.
- [x] Commit the completed slice.

## Verification

```sh
cargo test -p continuitydb-cli cli_replay_workload_require_manifest_rejects_report_path_mismatch
cargo test -p continuitydb-cli cli_replay_workload_require_manifest_rejects_fixture_path_mismatch
cargo fmt --all -- --check
git diff --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
```
