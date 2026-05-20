# Local Executable Model Runner Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a concrete local executable runner backend for `local-model` Steward inference.

**Architecture:** The runner implements `LocalModelBackend` and shells out to a configured local inference executable. It constructs arguments deterministically, sends the Steward prompt through stdin, captures stdout as the model JSON response, and converts process failures into `StewardError` without panics or hidden mutation.

**Tech Stack:** Rust 2021 standard library process APIs, existing `LocalModelBackend`, `LocalModelRequest`, and `StewardError`.

---

### Task 1: RED Runner Tests

**Files:**
- Modify: `crates/continuitydb-steward/src/lib.rs`

- [ ] **Step 1: Write failing tests**

Add feature-gated tests proving:
- runner configuration builds deterministic command arguments.
- runner can execute a local test script and return stdout.
- runner maps non-zero process status to a Steward error.

- [ ] **Step 2: Verify RED**

Run: `cargo test -p continuitydb-steward --features local-model local_executable`

Expected: compilation fails because local executable runner types are not exported yet.

### Task 2: Implement Runner Backend

**Files:**
- Modify: `crates/continuitydb-steward/src/error.rs`
- Modify: `crates/continuitydb-steward/src/local_model.rs`
- Modify: `crates/continuitydb-steward/src/lib.rs`

- [ ] **Step 1: Add runner types**

Implement:
- `LocalExecutableRunner`
- `LocalExecutableRunnerConfig`

`LocalExecutableRunner` must implement `LocalModelBackend`.

- [ ] **Step 2: Verify GREEN**

Run: `cargo test -p continuitydb-steward --features local-model local_executable`

Expected: new local executable runner tests pass.

### Task 3: Docs and Full Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [ ] **Step 1: Mark progress**

Update current scope and the local model milestone caveat to reflect that a local executable runner boundary exists, while model-specific benchmarking remains future work.

- [ ] **Step 2: Verify**

Run:
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace --all-features`
- `cargo test --workspace`

Expected: all checks pass.
