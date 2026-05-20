# CLI Workload Replay Bundle Separation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Prevent replay artifact bundles from being written into their input workload artifact directory.

**Architecture:** Add a focused CLI validation before replay artifact reads and writes begin. Keep explicit standalone report paths unaffected, and document the replay bundle separation invariant in the roadmap/current scope.

**Tech Stack:** Rust 2021, clap derive, serde_json, assert_cmd CLI tests.

---

### Task 1: Replay Bundle Directory Separation

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`
- Test: `crates/continuitydb-cli/tests/cli.rs`
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [x] **Step 1: Write the failing CLI test**

Add `cli_replay_workload_artifact_dir_rejects_input_directory` near the existing replay workload bundle tests. The test must create a workload bundle, run `replay-workload --artifact-dir <dir> --replay-artifact-dir <dir>`, assert failure with `--replay-artifact-dir must differ from --artifact-dir`, and assert `<dir>/continuitydb-workload-replay.manifest.json` does not exist.

- [x] **Step 2: Run the focused test to verify it fails**

Run: `cargo test -p continuitydb-cli cli_replay_workload_artifact_dir_rejects_input_directory`

Expected: FAIL because replay currently accepts the same input/output directory and writes the replay bundle.

- [x] **Step 3: Implement validation**

Add an early check in `replay_workload_json` that compares `options.replay_artifact_dir` with `options.artifact_dir` and returns `std::io::Error::other("--replay-artifact-dir must differ from --artifact-dir")` when they match.

- [x] **Step 4: Run focused and full verification**

Run:

```bash
cargo test -p continuitydb-cli cli_replay_workload_artifact_dir_rejects_input_directory
cargo test -p continuitydb-cli cli_replay_workload_artifact_dir_writes_mismatch_bundle
cargo fmt --all -- --check
git diff --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
```

- [x] **Step 5: Commit**

Commit message: `feat: separate workload replay bundles`
