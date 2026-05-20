# CLI Workload Replay Manifest Fixture Byte Count Validation Plan

## Goal

Validate archived fixture byte counts inside required workload replay manifests.

## Steps

- [x] Add a failing CLI test for a manifest whose fixture byte count no longer matches the archived file.
- [x] Validate manifest `cells_bytes`.
- [x] Validate manifest `checkout_request_bytes`.
- [x] Update README and roadmap milestone tracking.
- [x] Run focused and workspace verification gates.
- [x] Commit the completed slice.

## Verification

```sh
cargo test -p continuitydb-cli cli_replay_workload_require_manifest_rejects_fixture_byte_count_mismatch
cargo test -p continuitydb-cli cli_replay_workload_require_manifest_rejects_workload_summary_mismatch
cargo test -p continuitydb-cli cli_replay_workload_require_manifest_rejects_tampered_fixture
cargo fmt --all -- --check
git diff --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
```
