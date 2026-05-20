# CLI Workload Replay Input Manifest Metadata Plan

## Goal

Emit validated input bundle manifest metadata from manifest-required workload replays.

## Steps

- [x] Add a failing CLI test for successful `replay-workload --require-manifest` metadata in stdout and replay bundle manifests.
- [x] Return manifest metadata from workload artifact manifest validation.
- [x] Include `input_bundle_manifest` in replay reports and replay bundle manifests.
- [x] Update README and roadmap milestone tracking.
- [x] Run focused and workspace verification gates.
- [x] Commit the completed slice.

## Verification

```sh
cargo test -p continuitydb-cli cli_replay_workload_require_manifest_reports_validated_manifest
cargo test -p continuitydb-cli cli_replay_workload_require_manifest_rejects_tampered_fixture
cargo fmt --all -- --check
git diff --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
```
