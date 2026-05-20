# Workload Measurement Harness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a reusable measurement harness that runs deterministic workloads through a storage kernel and checkout.

**Architecture:** Extend `continuitydb-workload` with storage-kernel-generic measurement APIs. The harness appends a workload batch, runs a supplied `CheckoutRequest`, and reports deterministic operation counts plus observational durations.

**Tech Stack:** Rust 2021, `continuitydb-workload`, `continuitydb-kernel`, `continuitydb-checkout`, `continuitydb-memory` tests.

---

### Task 1: RED Measurement Tests

**Files:**
- Modify: `crates/continuitydb-workload/Cargo.toml`
- Modify: `crates/continuitydb-workload/src/lib.rs`

- [x] **Step 1: Add dependencies and failing tests**

Add `continuitydb-checkout` and `continuitydb-kernel` dependencies, plus `continuitydb-memory` as a dev-dependency. Add tests for successful memory-kernel measurement and duplicate-ingest error propagation.

- [x] **Step 2: Verify RED**

Run: `cargo test -p continuitydb-workload workload_measurement`

Expected: FAIL because `measure_ingest_and_checkout`, measurement structs, and `MeasurementError` do not exist.

### Task 2: GREEN Measurement Harness

**Files:**
- Modify: `crates/continuitydb-workload/src/lib.rs`

- [x] **Step 1: Implement measurement types and harness**

Implement `MeasuredOperation`, `CheckoutMeasurement`, `WorkloadMeasurement`, `MeasurementError`, and `measure_ingest_and_checkout`.

- [x] **Step 2: Verify GREEN**

Run: `cargo test -p continuitydb-workload workload_measurement`

Expected: PASS.

### Task 3: Docs, Full Gate, Commit

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-workload-measurement-harness.md`

- [x] **Step 1: Update docs**

Record workload measurement harness in README current scope and Benchmark and Workload milestones.

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

Commit with `feat: measure deterministic workload checkout`.
