# Local Model Benchmark Baselines Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add durable benchmark baseline records for executable local Steward model evaluations.

**Architecture:** Extend the feature-gated `local-model` benchmark layer with serializable baseline records and a pluggable append-only baseline store. Mirror the existing JSONL store patterns so future llama.cpp/GGUF or mistral.rs runs can be recorded and compared without adding real inference dependencies or hosted services.

**Tech Stack:** Rust 2021, `chrono`, `serde`, `serde_json`, existing `LocalModelBenchmarkReport`, `SmallModelCandidate`, and `StewardEvaluationReport`.

---

### Task 1: RED Baseline Tests

**Files:**
- Modify: `crates/continuitydb-steward/src/lib.rs`

- [ ] **Step 1: Write failing tests**

Add feature-gated tests proving:
- A `LocalModelBenchmarkBaseline` preserves candidate metadata, evaluation report, pass/fail status, and recorded timestamp.
- `MemoryLocalModelBenchmarkBaselineStore` lists baselines in insertion order.
- `FileLocalModelBenchmarkBaselineStore` persists baselines across reopen and rejects corrupt JSONL.

- [ ] **Step 2: Verify RED**

Run: `cargo test -p continuitydb-steward --features local-model local_model_benchmark_baseline`

Expected: compilation fails because baseline record and store types are not implemented or exported.

### Task 2: Implement Baseline Records and Stores

**Files:**
- Modify: `crates/continuitydb-steward/src/local_model.rs`
- Modify: `crates/continuitydb-steward/src/error.rs`
- Modify: `crates/continuitydb-steward/src/lib.rs`

- [ ] **Step 1: Add serializable evaluation types**

Derive `Serialize` and `Deserialize` where needed for:
- `StewardEvaluationReport`
- `StewardEvaluationCaseReport`
- `StewardEvaluationFailure`

- [ ] **Step 2: Add benchmark baseline types**

Implement:
- `LocalModelBenchmarkBaseline`
- `LocalModelBenchmarkBaselineStore`
- `MemoryLocalModelBenchmarkBaselineStore`
- `FileLocalModelBenchmarkBaselineStore`

The baseline should convert from `LocalModelBenchmarkReport` plus `recorded_at` and store candidate model ID and role as owned strings.

- [ ] **Step 3: Add store error variants**

Add `LocalModelBenchmarkBaselineStoreIo` and `LocalModelBenchmarkBaselineStoreCorrupt` to `StewardError`.

- [ ] **Step 4: Verify GREEN**

Run: `cargo test -p continuitydb-steward --features local-model local_model_benchmark_baseline`

Expected: all baseline tests pass.

### Task 3: Docs and Full Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [ ] **Step 1: Mark roadmap progress**

Update current scope and Steward milestone 6 to say benchmark baselines can now be recorded durably, while collecting real model result data remains future work.

- [ ] **Step 2: Verify**

Run:
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace --all-features`
- `cargo test --workspace`
- `git diff --check`

Expected: all checks pass.
