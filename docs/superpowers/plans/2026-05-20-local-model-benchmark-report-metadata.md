# Local Model Benchmark Report Metadata Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add benchmark report fingerprint and byte metadata to local model benchmark bundle manifests.

**Architecture:** Reuse the existing local-model artifact fingerprint helper. Read the already-written `benchmark-report.json` in `write_local_model_bundle_manifest`, then include report fingerprint and byte count beside the report path in the manifest JSON.

**Tech Stack:** Rust workspace, `continuitydb-cli`, serde JSON, assert_cmd, cargo tests.

---

### Task 1: Add benchmark report artifact metadata

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`
- Modify: `crates/continuitydb-cli/tests/cli.rs`
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [x] **Step 1: Write the failing CLI test**

Extend `cli_benchmark_local_model_artifact_dir_writes_real_run_bundle` to assert `local-model-benchmark.manifest.json` includes `benchmark_report_fingerprint` with an `fnv1a64:` prefix and `benchmark_report_bytes > 0`.

- [x] **Step 2: Run focused test to verify RED**

Run:

```sh
cargo test -p continuitydb-cli cli_benchmark_local_model_artifact_dir_writes_real_run_bundle --features local-model
```

Expected: failure because the bundle manifest does not yet contain benchmark report fingerprint or byte metadata.

- [x] **Step 3: Implement minimal production code**

Update `write_local_model_bundle_manifest` to read `benchmark-report.json` after it is written and add `benchmark_report_fingerprint` plus `benchmark_report_bytes` to the manifest JSON.

- [x] **Step 4: Run focused test to verify GREEN**

Run the same focused test and confirm it passes.

- [x] **Step 5: Update docs**

Record local model benchmark report metadata in `README.md` and `docs/roadmap.md`.

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
git add README.md crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs docs/roadmap.md docs/superpowers/plans/2026-05-20-local-model-benchmark-report-metadata.md docs/superpowers/specs/2026-05-20-local-model-benchmark-report-metadata-design.md
git commit -m "feat: record local-model benchmark report metadata"
```
