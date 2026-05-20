# CLI Workload Replay Manifest Artifact Directory Validation Plan

## Goal

Validate the archived artifact directory inside required workload replay manifests.

## Steps

- [x] Add a failing CLI test for a manifest whose `artifact_dir` no longer matches the replayed `--artifact-dir`.
- [x] Validate manifest `artifact_dir`.
- [x] Update README and roadmap milestone tracking.
- [x] Run focused and workspace verification gates.
- [x] Commit the completed slice.

## Verification

```sh
cargo test -p continuitydb-cli cli_replay_workload_require_manifest_rejects_artifact_dir_mismatch
cargo test -p continuitydb-cli cli_replay_workload_require_manifest_rejects_report_path_mismatch
cargo test -p continuitydb-cli cli_replay_workload_require_manifest_rejects_fixture_path_mismatch
cargo fmt --all -- --check
git diff --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
```
