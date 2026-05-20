# Local Model Runtime Profiles Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add deterministic runner profiles for llama.cpp and mistral.rs local Steward model benchmarking.

**Architecture:** Extend the feature-gated `local-model` boundary with small profile structs that produce `LocalExecutableRunnerConfig` values. The profiles should only describe executable arguments; they must not add inference dependencies, download models, or execute anything during configuration.

**Tech Stack:** Rust 2021, existing `LocalExecutableRunnerConfig`, `PathBuf`, and `local-model` feature.

---

### Task 1: RED Runtime Profile Tests

**Files:**
- Modify: `crates/continuitydb-steward/src/lib.rs`

- [ ] **Step 1: Write failing tests**

Add feature-gated tests proving:
- `LlamaCppRuntimeProfile` builds a deterministic `llama-cli` config with model path, context size, temperature, grammar file, and stdin prompt argument.
- `MistralRsRuntimeProfile` builds a deterministic `mistralrs-server`/CLI-style config with model path, context size, temperature, and JSON output flag.

- [ ] **Step 2: Verify RED**

Run: `cargo test -p continuitydb-steward --features local-model runtime_profile`

Expected: compilation fails because runtime profile types are not implemented or exported.

### Task 2: Implement Runtime Profiles

**Files:**
- Modify: `crates/continuitydb-steward/src/local_model.rs`
- Modify: `crates/continuitydb-steward/src/lib.rs`

- [ ] **Step 1: Add public profile types**

Implement:
- `LlamaCppRuntimeProfile`
- `MistralRsRuntimeProfile`

Each profile should expose `new`, builder methods for executable/model/options, getters where useful, and `runner_config()`.

- [ ] **Step 2: Verify GREEN**

Run: `cargo test -p continuitydb-steward --features local-model runtime_profile`

Expected: runtime profile tests pass.

### Task 3: Docs and Full Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [ ] **Step 1: Mark roadmap progress**

Update current scope and Steward milestone 5 to say deterministic llama.cpp and mistral.rs runner profiles exist, while real runtime execution and model-result collection remain future work.

- [ ] **Step 2: Verify**

Run:
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace --all-features`
- `cargo test --workspace`
- `git diff --check`

Expected: all checks pass.
