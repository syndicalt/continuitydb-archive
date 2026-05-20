# Workload Replay Report Metadata Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add replay report fingerprint and byte metadata to workload replay bundle manifests.

**Architecture:** Reuse the existing FNV-1a fingerprint helper. Read the just-written `replay-report.json` text when constructing the replay bundle manifest, then store its fingerprint and byte count beside the report path.

**Tech Stack:** Rust workspace, `continuitydb-cli`, serde JSON, assert_cmd, cargo tests.

---

### Task 1: Add replay report artifact metadata

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`
- Modify: `crates/continuitydb-cli/tests/cli.rs`
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [x] **Step 1: Write the failing CLI test**

Extend the replay artifact bundle test to assert `continuitydb-workload-replay.manifest.json` includes `replay_report_fingerprint` with an `fnv1a64:` prefix and `replay_report_bytes > 0`.

- [x] **Step 2: Run focused test to verify RED**

Run:

```sh
cargo test -p continuitydb-cli cli_replay_workload_artifact_dir_writes_mismatch_bundle
```

Expected: failure because the replay manifest does not yet contain replay report fingerprint or byte metadata.

- [x] **Step 3: Implement minimal production code**

Update `write_workload_replay_bundle_manifest` to read `replay-report.json` after it is written and add `replay_report_fingerprint` plus `replay_report_bytes` to the manifest JSON.

- [x] **Step 4: Run focused test to verify GREEN**

Run the same focused test and confirm it passes.

- [x] **Step 5: Update docs**

Record replay report metadata in `README.md` and `docs/roadmap.md`.

- [x] **Step 6: Run verification gates**

Run:

```sh
cargo fmt --all -- --check
git diff --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
```

- [x] **Step 7: Commit**

Commit:

```sh
git add README.md crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs docs/roadmap.md docs/superpowers/plans/2026-05-20-workload-replay-report-metadata.md docs/superpowers/specs/2026-05-20-workload-replay-report-metadata-design.md
git commit -m "feat: record workload replay report metadata"
```
