# Local Model Validation Report Failure Metadata Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Preserve best-effort benchmark report metadata in local-model validation failure reports.

**Architecture:** Add a local-model-specific benchmark report metadata helper beside the failure-report writer. The helper reads `benchmark-report.json`, uses canonical manifest payload text when possible, and falls back to raw file text when canonicalization is unavailable.

**Tech Stack:** Rust workspace, `continuitydb-cli`, `assert_cmd`, serde JSON, cargo tests with `local-model` feature.

---

### Task 1: Add benchmark report metadata to local-model validation failure reports

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`
- Modify: `crates/continuitydb-cli/tests/cli.rs`
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [x] **Step 1: Write the failing acceptance test**

Extend `cli_validate_local_model_bundle_failure_report_path_records_validation_failure` to assert:

```rust
let benchmark_report_path = artifact_dir.join("benchmark-report.json");
assert_eq!(
    failure_report["benchmark_report"]["report_path"].as_str(),
    Some(benchmark_report_path.display().to_string().as_str())
);
assert!(failure_report["benchmark_report"]["report_fingerprint"]
    .as_str()
    .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
assert!(failure_report["benchmark_report"]["report_bytes"]
    .as_u64()
    .is_some_and(|bytes| bytes > 0));
```

- [x] **Step 2: Run focused test to verify RED**

Run:

```sh
cargo test -p continuitydb-cli --features local-model cli_validate_local_model_bundle_failure_report_path_records_validation_failure --test cli
```

Expected: fail because `benchmark_report` is currently `null` in local-model validation failure reports.

- [x] **Step 3: Implement best-effort benchmark report metadata**

Add:

```rust
fn local_model_validation_failure_report_json(artifact_dir: &Path) -> serde_json::Value
```

Use it from `write_local_model_bundle_validation_failure_report`.

- [x] **Step 4: Run focused test to verify GREEN**

Run:

```sh
cargo test -p continuitydb-cli --features local-model cli_validate_local_model_bundle_failure_report_path_records_validation_failure --test cli
```

Expected: pass.

- [x] **Step 5: Update docs**

Add `CLI local model bundle validation report failure metadata` to `README.md` current scope and add the next local-model roadmap milestone in `docs/roadmap.md`.

- [x] **Step 6: Run verification gates**

Run:

```sh
cargo fmt --all
cargo fmt --all -- --check
git diff --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
```

- [x] **Step 7: Commit**

Commit:

```sh
git add README.md crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs docs/roadmap.md docs/superpowers/plans/2026-05-20-local-model-validation-report-failure-metadata.md docs/superpowers/specs/2026-05-20-local-model-validation-report-failure-metadata-design.md
git commit -m "feat: report local-model validation report evidence"
```
