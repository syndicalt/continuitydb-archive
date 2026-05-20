# CLI Workload Replay Manifest Workload Summary Validation Plan

## Goal

Validate the archived workload summary inside required workload replay manifests.

## Steps

- [x] Add a failing CLI test for a manifest whose `workload` summary no longer matches `workload-cells.json`.
- [x] Validate manifest `workload` against the cells artifact summary.
- [x] Update README and roadmap milestone tracking.
- [x] Run focused and workspace verification gates.
- [x] Commit the completed slice.

## Verification

```sh
cargo test -p continuitydb-cli cli_replay_workload_require_manifest_rejects_workload_summary_mismatch
cargo test -p continuitydb-cli cli_replay_workload_require_manifest_rejects_artifact_dir_mismatch
cargo test -p continuitydb-cli cli_replay_workload_require_manifest_rejects_tampered_fixture
cargo fmt --all -- --check
git diff --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
```
