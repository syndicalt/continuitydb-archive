# CLI Workload Replay Report Artifacts Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add durable JSON report artifact output for `replay-workload`, including mismatch failure reports.

**Architecture:** Extend the CLI command surface with optional report paths, add those paths to replay JSON, and write the JSON artifact before returning mismatch failures. Keep replay execution deterministic and leave workload measurement artifacts unchanged.

**Tech Stack:** Rust 2021, clap derive, serde_json, assert_cmd CLI tests.

---

### Task 1: Replay Failure Report Artifact

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`
- Test: `crates/continuitydb-cli/tests/cli.rs`
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [x] **Step 1: Write the failing CLI test**

Add `cli_replay_workload_failure_report_path_records_mismatch` near the existing replay workload tests.

- [x] **Step 2: Run the focused test to verify it fails**

Run: `cargo test -p continuitydb-cli cli_replay_workload_failure_report_path_records_mismatch`

Expected: FAIL because `replay-workload` does not yet accept `--failure-report-path`.

- [x] **Step 3: Implement the minimal CLI support**

Add `--report-path` and `--failure-report-path` to `ReplayWorkload`, include both paths in replay JSON, write `--report-path` on success, and write `--failure-report-path` before returning `workload replay mismatch detected`.

- [x] **Step 4: Run focused and full verification**

Run:

```bash
cargo test -p continuitydb-cli cli_replay_workload_failure_report_path_records_mismatch
cargo test -p continuitydb-cli cli_replay_workload_compares_archived_report
cargo fmt --all -- --check
git diff --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
```

- [x] **Step 5: Commit**

Commit message: `feat: write workload replay reports`
