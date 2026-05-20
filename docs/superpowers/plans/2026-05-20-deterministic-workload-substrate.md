# Deterministic Workload Substrate Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add deterministic world-model workload generation so storage-engine and checkout benchmark work can use stable StateCell corpora.

**Architecture:** Add a new `continuitydb-workload` crate that depends on `continuitydb-core` and `chrono`. Add a small deterministic `StateCellId::from_u128` constructor in core so generated workloads are stable across runs without exposing UUID internals.

**Tech Stack:** Rust 2021, `continuitydb-core`, `chrono`, workspace Cargo.

---

### Task 1: RED Deterministic ID Primitive

**Files:**
- Modify: `crates/continuitydb-core/src/cell.rs`
- Modify: `crates/continuitydb-core/src/lib.rs`

- [x] **Step 1: Add failing core tests**

Add tests proving `StateCellId::from_u128` creates stable display/parse round trips and rejects no valid `u128` input.

- [x] **Step 2: Verify RED**

Run: `cargo test -p continuitydb-core state_cell_id_from_u128_is_stable`

Expected: FAIL because `StateCellId::from_u128` does not exist.

### Task 2: GREEN Deterministic ID Primitive

**Files:**
- Modify: `crates/continuitydb-core/src/cell.rs`

- [x] **Step 1: Implement `StateCellId::from_u128`**

Add a public constructor that wraps `Uuid::from_u128`.

- [x] **Step 2: Verify GREEN**

Run: `cargo test -p continuitydb-core state_cell_id_from_u128_is_stable`

Expected: PASS.

### Task 3: RED Workload Crate

**Files:**
- Modify: `Cargo.toml`
- Create: `crates/continuitydb-workload/Cargo.toml`
- Create: `crates/continuitydb-workload/src/lib.rs`

- [x] **Step 1: Add crate manifest and failing tests**

Add `continuitydb-workload` to the workspace and write tests for repeatable workload generation, frontier/dependency coverage, and invalid config errors.

- [x] **Step 2: Verify RED**

Run: `cargo test -p continuitydb-workload`

Expected: FAIL because the workload API is not implemented yet.

### Task 4: GREEN Workload Generator

**Files:**
- Modify: `crates/continuitydb-workload/src/lib.rs`

- [x] **Step 1: Implement config, errors, summary, and generator**

Implement `WorkloadConfig`, `WorkloadError`, `WorkloadSummary`, `ContinuityWorkload`, and `generate_world_model_workload`.

- [x] **Step 2: Verify GREEN**

Run: `cargo test -p continuitydb-workload`

Expected: PASS.

### Task 5: Docs, Full Gate, Commit

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-deterministic-workload-substrate.md`

- [x] **Step 1: Update docs**

Record deterministic workload generation in README current scope and roadmap Benchmark and Workload milestones.

- [x] **Step 2: Run full verification**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

- [x] **Step 3: Commit**

Commit with `feat: add deterministic workload substrate`.
