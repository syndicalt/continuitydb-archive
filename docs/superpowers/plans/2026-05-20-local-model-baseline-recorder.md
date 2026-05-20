# Local Model Baseline Recorder Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a production-shaped API that runs a configured local model benchmark and records the resulting baseline in an append-only store.

**Architecture:** Keep model execution behind the existing `LocalExecutableRunner` and benchmark suite. Add a small recorder function that runs `LocalModelBenchmark`, converts the report to `LocalModelBenchmarkBaseline`, appends it to any `LocalModelBenchmarkBaselineStore`, and returns the recorded baseline.

**Tech Stack:** Rust 2021, `continuitydb-steward` with the `local-model` feature, existing JSONL and in-memory baseline stores.

---

### Task 1: RED Baseline Recording Test

**Files:**
- Modify: `crates/continuitydb-steward/src/lib.rs`

- [x] **Step 1: Write the failing test**

Add a feature-gated test named `local_model_benchmark_records_baseline_in_store` near the existing local model benchmark baseline tests. The test should:
- Build a deterministic `sh` local model script returning one valid `MarkFrontier` proposal.
- Build a `LocalModelBenchmark` with `small_model_candidates()[0]`.
- Create a `MemoryLocalModelBenchmarkBaselineStore`.
- Call `record_local_model_benchmark_baseline(&benchmark, steward()?, created_at(), &mut store)`.
- Assert the returned baseline passed, includes the Qwen2.5 candidate metadata, and is listed in the store.

- [x] **Step 2: Verify RED**

Run:

```bash
cargo test -p continuitydb-steward --features local-model records_baseline
```

Expected: compilation fails because `record_local_model_benchmark_baseline` is not implemented/exported.

### Task 2: Implement Recorder API

**Files:**
- Modify: `crates/continuitydb-steward/src/local_model.rs`
- Modify: `crates/continuitydb-steward/src/lib.rs`

- [x] **Step 1: Add recorder function**

Add this function to `local_model.rs` near `LocalModelBenchmarkBaseline`:

```rust
pub fn record_local_model_benchmark_baseline<S>(
    benchmark: &LocalModelBenchmark,
    identity: StewardIdentity,
    recorded_at: DateTime<Utc>,
    store: &mut S,
) -> Result<LocalModelBenchmarkBaseline, StewardError>
where
    S: LocalModelBenchmarkBaselineStore,
{
    let baseline = LocalModelBenchmarkBaseline::from_report(benchmark.run(identity), recorded_at);
    store.append_baseline(baseline.clone())?;
    Ok(baseline)
}
```

- [x] **Step 2: Export the recorder**

Add `record_local_model_benchmark_baseline` to the `#[cfg(feature = "local-model")] pub use local_model::{...}` list in `lib.rs`, and to the test imports.

- [x] **Step 3: Verify GREEN**

Run:

```bash
cargo test -p continuitydb-steward --features local-model records_baseline
```

Expected: the new recorder test passes.

### Task 3: Roadmap and Verification

**Files:**
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-local-model-baseline-recorder.md`

- [x] **Step 1: Update roadmap milestone 6**

Change milestone 6 to say the evaluation harness now includes a baseline recorder for configured real-runtime runs, while collecting environment-specific real model baseline artifacts remains future work.

- [x] **Step 2: Mark this plan complete**

Check off completed steps in this plan before commit.

- [x] **Step 3: Verify**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

Expected: all checks pass.
