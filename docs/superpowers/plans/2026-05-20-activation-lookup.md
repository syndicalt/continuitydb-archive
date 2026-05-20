# Activation Lookup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add activation-state filtering to `CellLookup` so kernels can retrieve active, frontier, retired, or dormant StateCells deterministically.

**Architecture:** Extend the shared kernel query contract with an optional `ActivationState` filter. Apply the same filtering semantics in both `FileKernel` and `MemoryKernel`.

**Tech Stack:** Rust 2021, existing `ActivationState` enum in `continuitydb-core`.

---

### Task 1: RED Activation Filter Tests

**Files:**
- Modify: `crates/continuitydb-kernel/src/lib.rs`
- Modify: `crates/continuitydb-memory/src/lib.rs`

- [x] **Step 1: Write failing file-kernel test**

Add `file_kernel_filters_by_activation_state`. It should append two cells with the same scope but different activations, then lookup with `activation: Some(ActivationState::Frontier)` and assert only the frontier cell is returned.

- [x] **Step 2: Write failing memory-kernel test**

Add `memory_kernel_filters_by_activation_state` with the same behavior for `MemoryKernel`.

- [x] **Step 3: Verify RED**

Run:

```bash
cargo test -p continuitydb-kernel activation_state
cargo test -p continuitydb-memory activation_state
```

Expected: compilation fails because `CellLookup` has no `activation` field.

### Task 2: Implement Activation Filtering

**Files:**
- Modify: `crates/continuitydb-kernel/src/lib.rs`
- Modify: `crates/continuitydb-memory/src/lib.rs`
- Modify: `docs/roadmap.md`

- [x] **Step 1: Extend `CellLookup`**

Import `ActivationState` in `continuitydb-kernel`, and add:

```rust
pub activation: Option<ActivationState>,
```

- [x] **Step 2: Filter `FileKernel` lookup**

Add an activation filter to `FileKernel::lookup_cells`.

- [x] **Step 3: Filter `MemoryKernel` lookup**

Add the same activation filter to `MemoryKernel::lookup_cells`.

- [x] **Step 4: Verify GREEN**

Run:

```bash
cargo test -p continuitydb-kernel activation_state
cargo test -p continuitydb-memory activation_state
```

Expected: activation filter tests pass.

### Task 3: Roadmap and Verification

**Files:**
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-activation-lookup.md`

- [x] **Step 1: Update roadmap**

Update the storage-kernel milestone to include activation-state filtering.

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
