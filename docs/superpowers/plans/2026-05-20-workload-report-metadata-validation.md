# Workload Report Metadata Validation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `replay-workload --require-manifest` validate workload report fingerprint and byte metadata.

**Architecture:** Reuse the existing manifest validator. After validating `workload_report_path`, read `workload-report.json`, parse it, normalize `bundle_manifest` to `null`, compare the manifest-owned byte count and FNV-1a fingerprint against that canonical report payload, then continue existing report-content validation.

**Tech Stack:** Rust workspace, `continuitydb-cli`, serde JSON, assert_cmd, cargo tests.

---

### Task 1: Enforce workload report metadata during replay manifest validation

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`
- Modify: `crates/continuitydb-cli/tests/cli.rs`
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [x] **Step 1: Write failing CLI tests**

Add two tests:

```rust
fn cli_replay_workload_require_manifest_rejects_report_byte_count_mismatch()
fn cli_replay_workload_require_manifest_rejects_report_fingerprint_mismatch()
```

Each test should create a workload artifact bundle, mutate only the relevant root report metadata field in `continuitydb-workload.manifest.json`, run `replay-workload --require-manifest`, and assert the expected validation error.

- [x] **Step 2: Run focused tests to verify RED**

Run:

```sh
cargo test -p continuitydb-cli cli_replay_workload_require_manifest_rejects_report_byte_count_mismatch
cargo test -p continuitydb-cli cli_replay_workload_require_manifest_rejects_report_fingerprint_mismatch
```

Expected: failures because report metadata is not yet validated.

- [x] **Step 3: Implement minimal production code**

Update `write_workload_bundle_manifest` and `validate_workload_artifact_manifest` to compare `workload_report_bytes` and `workload_report_fingerprint` against the canonical workload report payload with `bundle_manifest` normalized to `null`.

- [x] **Step 4: Run focused tests to verify GREEN**

Run the same focused tests and confirm they pass.

- [x] **Step 5: Update docs**

Record workload report metadata validation in `README.md` and `docs/roadmap.md`.

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
git add README.md crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs docs/roadmap.md docs/superpowers/plans/2026-05-20-workload-report-metadata-validation.md docs/superpowers/specs/2026-05-20-workload-report-metadata-validation-design.md
git commit -m "feat: validate workload report metadata"
```
