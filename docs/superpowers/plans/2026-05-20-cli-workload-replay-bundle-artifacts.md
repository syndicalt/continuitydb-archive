# CLI Workload Replay Bundle Artifacts Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add archiveable replay output bundles for `replay-workload` success and mismatch failure evidence.

**Architecture:** Treat the existing workload artifact directory as immutable input and add a separate replay artifact output directory. Reuse existing pretty JSON and FNV fingerprint helpers to write `replay-report.json`, create a versioned replay manifest, then rewrite the report with manifest metadata so the bundle is self-describing.

**Tech Stack:** Rust 2021, clap derive, serde_json, assert_cmd CLI tests.

---

### Task 1: Replay Bundle Artifact Output

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`
- Test: `crates/continuitydb-cli/tests/cli.rs`
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [x] **Step 1: Write the failing CLI test**

Add `cli_replay_workload_artifact_dir_writes_mismatch_bundle` near the existing replay workload tests. The test must create a workload input bundle, mutate its archived `workload-report.json`, run `replay-workload --fail-on-mismatch --replay-artifact-dir <dir>`, assert the command fails with `workload replay mismatch detected`, and verify `replay-report.json` plus `continuitydb-workload-replay.manifest.json`.

- [x] **Step 2: Run the focused test to verify it fails**

Run: `cargo test -p continuitydb-cli cli_replay_workload_artifact_dir_writes_mismatch_bundle`

Expected: FAIL because `replay-workload` does not yet accept `--replay-artifact-dir`.

- [x] **Step 3: Implement replay bundle output**

Add the `--replay-artifact-dir` CLI option, include `replay_artifact_dir` and `replay_bundle_manifest` fields in replay JSON, and write the replay bundle before returning mismatch failures or successful output.

- [x] **Step 4: Run focused and full verification**

Run:

```bash
cargo test -p continuitydb-cli cli_replay_workload_artifact_dir_writes_mismatch_bundle
cargo test -p continuitydb-cli cli_replay_workload_failure_report_path_records_mismatch
cargo test -p continuitydb-cli cli_replay_workload_replays_artifact_bundle
cargo fmt --all -- --check
git diff --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
```

- [x] **Step 5: Commit**

Commit message: `feat: bundle workload replay reports`
