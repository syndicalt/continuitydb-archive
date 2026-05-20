# CLI Workload Replay Manifest Path Validation Plan

## Goal

Validate fixture paths inside required workload replay manifests.

## Steps

- [x] Add a failing CLI test for a manifest whose `cells_path` no longer matches `<artifact-dir>/workload-cells.json`.
- [x] Validate manifest `cells_path`.
- [x] Validate manifest `checkout_request_path`.
- [x] Update README and roadmap milestone tracking.
- [x] Run focused and workspace verification gates.
- [ ] Commit the completed slice.

## Verification

```sh
cargo test -p continuitydb-cli cli_replay_workload_require_manifest_rejects_fixture_path_mismatch
cargo test -p continuitydb-cli cli_replay_workload_require_manifest_rejects_tampered_fixture
cargo test -p continuitydb-cli cli_replay_workload_require_manifest_writes_validation_failure_bundle
cargo fmt --all -- --check
git diff --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
```
