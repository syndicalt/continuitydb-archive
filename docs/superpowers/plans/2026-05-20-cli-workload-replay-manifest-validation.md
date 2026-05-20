# CLI Workload Replay Manifest Validation Plan

## Goal

Add opt-in manifest validation for replayed workload artifact bundles.

## Steps

- [x] Add a failing CLI test for `replay-workload --require-manifest` rejecting a tampered `workload-cells.json`.
- [x] Add the `--require-manifest` replay option and wire it into replay execution.
- [x] Validate workload bundle manifest format, version, and fixture fingerprints before replay.
- [x] Update README and roadmap milestone tracking.
- [x] Run focused and workspace verification gates.
- [x] Commit the completed slice.

## Verification

```sh
cargo test -p continuitydb-cli cli_replay_workload_require_manifest_rejects_tampered_fixture
cargo test -p continuitydb-cli cli_replay_workload_replays_artifact_bundle
cargo fmt --all -- --check
git diff --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
```
