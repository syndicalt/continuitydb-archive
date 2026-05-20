# Steward Evaluation Harness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add fixed proposal-quality evaluation tests for small open-source Steward model candidates.

**Architecture:** The evaluation harness lives behind the existing `local-model` feature in `continuitydb-steward`. It runs a `LocalModelSteward` against deterministic cases, scores the decoded proposals with explicit failure reasons, and exposes static candidate metadata for the small embeddable models already on the roadmap.

**Tech Stack:** Rust 2021, existing `LocalModelBackend`, `LocalModelSteward`, `StewardAction`, `ProposalPolicy`, and `serde_json` feature path.

---

### Task 1: RED Evaluation Tests

**Files:**
- Modify: `crates/continuitydb-steward/src/lib.rs`

- [ ] **Step 1: Write failing tests**

Add feature-gated tests proving:
- Evaluation passes when model output preserves citations, classifies conflict links correctly, and avoids unsupported rationale terms.
- Evaluation fails with deterministic reason codes when citations are dropped or unsupported claims appear.
- Recommended small model candidates include Qwen2.5-0.5B-Instruct as the default feasibility candidate.

- [ ] **Step 2: Verify RED**

Run: `cargo test -p continuitydb-steward --features local-model`

Expected: compilation fails because evaluation types and candidate metadata are not exported.

### Task 2: Implement Evaluation Harness

**Files:**
- Modify: `crates/continuitydb-steward/src/local_model.rs`
- Modify: `crates/continuitydb-steward/src/lib.rs`

- [ ] **Step 1: Add public evaluation types**

Implement:
- `StewardEvaluationCase`
- `StewardEvaluationSuite`
- `StewardEvaluationReport`
- `StewardEvaluationCaseReport`
- `StewardEvaluationFailure`
- `SmallModelCandidate`
- `small_model_candidates()`

- [ ] **Step 2: Verify GREEN**

Run: `cargo test -p continuitydb-steward --features local-model`

Expected: all Steward tests pass with the feature enabled.

### Task 3: Docs and Full Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [ ] **Step 1: Mark roadmap progress**

Update the current scope and milestone 6 to say the fixed evaluation harness exists, while real model runner benchmarking remains future work.

- [ ] **Step 2: Verify**

Run:
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace --all-features`
- `cargo test --workspace`

Expected: all checks pass.
