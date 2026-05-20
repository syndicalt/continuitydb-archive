# Local Model Steward Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the first feature-gated local model inference boundary for the database Steward.

**Architecture:** The Steward crate keeps deterministic proposal semantics at the commit boundary. A `local-model` feature exposes a backend trait, prompt/input types, and JSON proposal decoding so local inference engines can propose Steward actions without directly mutating database state.

**Tech Stack:** Rust 2021, `serde`, optional `serde_json`, existing `continuitydb-steward` proposal and policy types.

---

### Task 1: Feature Flag and Contract Tests

**Files:**
- Modify: `crates/continuitydb-steward/Cargo.toml`
- Modify: `crates/continuitydb-steward/src/lib.rs`

- [ ] **Step 1: Add failing tests**

Add feature-gated tests for:
- A model Steward invoking a backend once.
- JSON output decoding into proposals.
- Invalid JSON returning a Steward error.

- [ ] **Step 2: Run the feature tests to verify RED**

Run: `cargo test -p continuitydb-steward --features local-model`

Expected: compilation fails because the `local_model` module and exported types do not exist yet.

### Task 2: Local Model Module

**Files:**
- Create: `crates/continuitydb-steward/src/local_model.rs`
- Modify: `crates/continuitydb-steward/src/error.rs`
- Modify: `crates/continuitydb-steward/src/lib.rs`

- [ ] **Step 1: Implement the module**

Add:
- `LocalModelBackend` trait.
- `LocalModelRequest`.
- `LocalModelSteward`.
- `LocalModelStewardInput`.
- JSON proposal response decoding into existing `StewardProposal` values.

- [ ] **Step 2: Run the feature tests to verify GREEN**

Run: `cargo test -p continuitydb-steward --features local-model`

Expected: all Steward tests pass with the feature enabled.

### Task 3: Docs and Workspace Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [ ] **Step 1: Mark roadmap progress**

Update the current scope and Steward milestone 5 to reflect that the local model inference boundary exists behind `local-model`.

- [ ] **Step 2: Verify**

Run:
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace --all-features`

Expected: all checks pass.
