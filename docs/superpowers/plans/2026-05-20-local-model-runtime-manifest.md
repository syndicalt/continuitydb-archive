# Local Model Runtime Manifest Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Store reproducible local executable invocation metadata in local-model benchmark reports and baselines.

**Architecture:** Add a serializable `LocalModelRuntimeManifest`, populate it from `LocalExecutableRunnerConfig` inside `LocalModelBenchmark::run`, and copy it into `LocalModelBenchmarkBaseline::from_report`. Keep legacy baseline JSON readable by giving missing runtime metadata a default empty manifest.

**Tech Stack:** Rust 2021, `continuitydb-steward` with the `local-model` feature, serde.

---

### Task 1: RED Tests

**Files:**
- Modify: `crates/continuitydb-steward/src/lib.rs`

- [x] **Step 1: Add report runtime manifest test**

Add `local_model_benchmark_report_preserves_runtime_manifest`. It should create a `LocalModelBenchmark` with an empty suite and a `LocalExecutableRunnerConfig` using executable `llama-cli`, model path `/models/qwen.gguf`, and arguments `--temp 0`. It should assert the report runtime executable and deterministic argument list.

- [x] **Step 2: Add baseline runtime manifest test**

Add `local_model_benchmark_baseline_preserves_runtime_manifest`. It should create a report through a configured benchmark, convert it to a baseline, and assert the baseline runtime manifest matches the report.

- [x] **Step 3: Add legacy JSON compatibility test**

Add `local_model_benchmark_baseline_decodes_legacy_json_without_runtime_manifest`. It should deserialize JSON containing candidate metadata, empty evaluation, and timestamp but no runtime manifest, then assert runtime executable is empty and arguments are empty.

- [x] **Step 4: Run RED focused tests**

Run:

```bash
cargo test -p continuitydb-steward local_model_benchmark_report_preserves_runtime_manifest --features local-model
cargo test -p continuitydb-steward local_model_benchmark_baseline_preserves_runtime_manifest --features local-model
cargo test -p continuitydb-steward local_model_benchmark_baseline_decodes_legacy_json_without_runtime_manifest --features local-model
```

Expected: FAIL before implementation because runtime manifest APIs do not exist.

### Task 2: GREEN Implementation

**Files:**
- Modify: `crates/continuitydb-steward/src/local_model.rs`

- [x] **Step 1: Add runtime manifest type**

Add `LocalModelRuntimeManifest` with `Clone`, `Debug`, `Default`, `Eq`, `PartialEq`, `Serialize`, and `Deserialize`. Add `from_runner_config`, `executable`, and `arguments` accessors.

- [x] **Step 2: Add runtime to reports**

Add `runtime: LocalModelRuntimeManifest` to `LocalModelBenchmarkReport`, populate it from `self.runner.config()` in `LocalModelBenchmark::run`, and expose `runtime()`.

- [x] **Step 3: Add runtime to baselines**

Add `runtime: LocalModelRuntimeManifest` to `LocalModelBenchmarkBaseline` with `#[serde(default)]`, copy `report.runtime` in `from_report`, and expose `runtime()`.

- [x] **Step 4: Run GREEN focused tests**

Run the same focused tests from Task 1. Expected: PASS.

### Task 3: Docs and Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-local-model-runtime-manifest.md`

- [x] **Step 1: Update README scope**

Add a current-scope bullet for reproducible local model benchmark runtime manifests.

- [x] **Step 2: Update roadmap**

Add a Steward milestone noting durable local-model baselines now preserve runtime invocation metadata.

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
git add README.md docs/roadmap.md docs/superpowers/specs/2026-05-20-local-model-runtime-manifest-design.md docs/superpowers/plans/2026-05-20-local-model-runtime-manifest.md crates/continuitydb-steward/src/local_model.rs crates/continuitydb-steward/src/lib.rs
git commit -m "feat: preserve local model runtime manifests"
```
