# Local Model Changed-Case Report Content Validation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `validate-local-model-bundle --artifact-dir` reject changed-case reports whose content no longer matches the archived benchmark report.

**Architecture:** Pass the parsed benchmark report into the changed-case report validator. After existing path, byte-count, and fingerprint checks pass, parse `changed-cases.json` and compare its compact projection against the fields generated from `benchmark_report["baseline_comparison"]`.

**Tech Stack:** Rust workspace, `continuitydb-cli`, feature-gated local-model CLI tests, serde JSON, assert_cmd.

---

### Task 1: Validate changed-case report content

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`
- Modify: `crates/continuitydb-cli/tests/cli.rs`
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [x] **Step 1: Write failing CLI tests**

Add feature-gated Unix tests:

```rust
fn cli_validate_local_model_bundle_rejects_changed_case_report_candidate_mismatch()
fn cli_validate_local_model_bundle_rejects_changed_case_report_comparison_mismatch()
```

Each test should create a changed-case bundle with `write_changed_case_local_model_bundle`, mutate `changed-cases.json`, refresh the root manifest's changed-case report metadata, and assert:

```rust
.stderr(contains("local model changed-case report content mismatch"))
```

- [x] **Step 2: Run focused tests to verify RED**

Run:

```sh
cargo test -p continuitydb-cli --features local-model cli_validate_local_model_bundle_rejects_changed_case_report_candidate_mismatch
cargo test -p continuitydb-cli --features local-model cli_validate_local_model_bundle_rejects_changed_case_report_comparison_mismatch
```

Expected: both tests fail because changed-case report content is not yet validated.

- [x] **Step 3: Implement minimal production code**

Update `validate_local_model_bundle_manifest` to call:

```rust
validate_local_model_changed_case_report_manifest(artifact_dir, &manifest, &report)
```

Add helpers:

```rust
fn local_model_changed_case_report_projection(
    benchmark_report: &serde_json::Value,
) -> serde_json::Value
```

```rust
fn validate_local_model_changed_case_report_content(
    report_path: &Path,
    changed_case_report: &serde_json::Value,
    benchmark_report: &serde_json::Value,
) -> Result<(), Box<dyn std::error::Error>>
```

The validator should compare format metadata, candidate identity, baseline path, report path, and compact comparison projection.

- [x] **Step 4: Run focused tests to verify GREEN**

Run:

```sh
cargo test -p continuitydb-cli --features local-model cli_validate_local_model_bundle_rejects_changed_case_report_candidate_mismatch
cargo test -p continuitydb-cli --features local-model cli_validate_local_model_bundle_rejects_changed_case_report_comparison_mismatch
```

Expected: both tests pass.

- [x] **Step 5: Update docs**

Record changed-case report content validation in `README.md` and `docs/roadmap.md`.

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
git add README.md crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs docs/roadmap.md docs/superpowers/plans/2026-05-20-local-model-changed-case-report-content-validation.md docs/superpowers/specs/2026-05-20-local-model-changed-case-report-content-validation-design.md
git commit -m "feat: validate local-model changed-case content"
```
