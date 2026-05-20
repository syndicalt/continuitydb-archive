# Local Model Benchmark Fixture Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a reusable benchmark fixture that runs an executable local Steward model against fixed proposal-quality evaluation suites.

**Architecture:** The benchmark fixture lives behind the existing `local-model` feature in `continuitydb-steward`. It composes `SmallModelCandidate`, `LocalExecutableRunner`, and `StewardEvaluationSuite` without downloading models or binding ContinuityDB to a specific inference runtime.

**Tech Stack:** Rust 2021, existing `LocalExecutableRunner`, `LocalModelSteward`, `StewardEvaluationSuite`, and small model candidate metadata.

---

### Task 1: RED Benchmark Tests

**Files:**
- Modify: `crates/continuitydb-steward/src/lib.rs`

- [ ] **Step 1: Write failing tests**

Add feature-gated tests proving:
- A `LocalModelBenchmark` can run a shell-backed executable runner through a fixed evaluation suite and preserve candidate metadata.
- The benchmark report preserves deterministic suite failures when a runner emits unsupported or under-cited proposals.

- [ ] **Step 2: Verify RED**

Run: `cargo test -p continuitydb-steward --features local-model local_model_benchmark`

Expected: compilation fails because `LocalModelBenchmark` is not implemented or exported.

### Task 2: Implement Benchmark Fixture

**Files:**
- Modify: `crates/continuitydb-steward/src/local_model.rs`
- Modify: `crates/continuitydb-steward/src/lib.rs`

- [ ] **Step 1: Add public benchmark types**

Implement:
- `LocalModelBenchmark`
- `LocalModelBenchmarkReport`

The benchmark should construct a `LocalModelSteward` from the supplied identity and executable runner, evaluate the suite, and return the candidate plus evaluation report.

- [ ] **Step 2: Verify GREEN**

Run: `cargo test -p continuitydb-steward --features local-model local_model_benchmark`

Expected: benchmark tests pass with the feature enabled.

### Task 3: Docs and Full Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [ ] **Step 1: Mark roadmap progress**

Update current scope and Steward milestone 6 to say the executable benchmark fixture exists, while real model result baselines remain future work.

- [ ] **Step 2: Verify**

Run:
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace --all-features`
- `cargo test --workspace`

Expected: all checks pass.
