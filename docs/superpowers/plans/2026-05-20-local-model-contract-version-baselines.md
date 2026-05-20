# Local Model Contract Version Baselines Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Store the local Steward response schema version in local-model benchmark reports, baselines, and CLI benchmark summaries.

**Architecture:** Add a `response_schema_version` field at the report boundary, copy it into durable baseline records, default missing legacy JSON to `0`, and include the value in `benchmark-local-model` JSON output. This keeps the version attached to the artifact that was actually produced.

**Tech Stack:** Rust 2021, serde, clap, `continuitydb-steward` with `local-model`.

---

### Task 1: RED Tests

**Files:**
- Modify: `crates/continuitydb-steward/src/lib.rs`
- Modify: `crates/continuitydb-cli/tests/cli.rs`

- [x] **Step 1: Add report schema-version test**

Add `local_model_benchmark_report_preserves_response_schema_version`. It should run an empty benchmark and assert `report.response_schema_version() == LOCAL_MODEL_RESPONSE_SCHEMA_VERSION`.

- [x] **Step 2: Add baseline schema-version test**

Add `local_model_benchmark_baseline_preserves_response_schema_version`. It should convert a benchmark report to a baseline and assert the baseline version matches `LOCAL_MODEL_RESPONSE_SCHEMA_VERSION`.

- [x] **Step 3: Extend legacy JSON compatibility test**

Update `local_model_benchmark_baseline_decodes_legacy_json_without_runtime_manifest` to also assert `baseline.response_schema_version() == 0`.

- [x] **Step 4: Extend CLI benchmark test**

Update `cli_benchmark_local_model_records_baseline` to assert output JSON `response_schema_version` is `1` and stored baseline JSON `response_schema_version` is `1`.

- [x] **Step 5: Run RED focused tests**

Run:

```bash
cargo test -p continuitydb-steward local_model_benchmark_report_preserves_response_schema_version --features local-model
cargo test -p continuitydb-steward local_model_benchmark_baseline_preserves_response_schema_version --features local-model
cargo test -p continuitydb-steward local_model_benchmark_baseline_decodes_legacy_json_without_runtime_manifest --features local-model
cargo test -p continuitydb-cli cli_benchmark_local_model_records_baseline --features local-model
```

Expected: FAIL before implementation because response schema version accessors/output fields do not exist.

### Task 2: GREEN Implementation

**Files:**
- Modify: `crates/continuitydb-steward/src/local_model.rs`
- Modify: `crates/continuitydb-cli/src/main.rs`

- [x] **Step 1: Add report field and accessor**

Add `response_schema_version: u32` to `LocalModelBenchmarkReport`, populate it with `LOCAL_MODEL_RESPONSE_SCHEMA_VERSION`, and add `response_schema_version()`.

- [x] **Step 2: Add baseline field and accessor**

Add `#[serde(default)] response_schema_version: u32` to `LocalModelBenchmarkBaseline`, copy `report.response_schema_version`, and add `response_schema_version()`.

- [x] **Step 3: Add CLI output field**

Add `response_schema_version: baseline.response_schema_version()` to `local_model_benchmark_json`.

- [x] **Step 4: Run GREEN focused tests**

Run the focused tests from Task 1. Expected: PASS.

### Task 3: Docs and Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-local-model-contract-version-baselines.md`

- [x] **Step 1: Update README scope**

Add a scope bullet for local model benchmark response contract versioning.

- [x] **Step 2: Update roadmap**

Add a Steward milestone noting that reports and baselines preserve response schema version metadata.

- [x] **Step 3: Run full verification gate**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

Expected: PASS.

- [x] **Step 4: Mark plan complete**

Check off all completed boxes in this plan.

- [x] **Step 5: Commit**

Commit with:

```bash
git add README.md docs/roadmap.md docs/superpowers/specs/2026-05-20-local-model-contract-version-baselines-design.md docs/superpowers/plans/2026-05-20-local-model-contract-version-baselines.md crates/continuitydb-steward/src/local_model.rs crates/continuitydb-steward/src/lib.rs crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs
git commit -m "feat: version local model benchmark contracts"
```
