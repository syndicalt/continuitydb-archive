# System Time Lookup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make StateCells bitemporal in practice by recording system transaction time and exposing system-time lookup constraints through storage kernels.

**Architecture:** Reuse the existing `SystemTimeRange` domain type and attach it to `StateCell` with serde defaults for older JSONL records. Extend `StorageKernel` with deterministic `append_cell_at` for tests and a default `append_cell` that stamps `Utc::now()`. Add `CellLookup.system_at` and apply it in memory and file kernels alongside existing valid-time filtering.

**Tech Stack:** Rust 2021, `chrono`, `serde`, `continuitydb-core`, `continuitydb-kernel`, `continuitydb-memory`.

---

### Task 1: StateCell System Time

**Files:**
- Modify: `crates/continuitydb-core/src/time.rs`
- Modify: `crates/continuitydb-core/src/cell.rs`
- Modify: `crates/continuitydb-kernel/src/lib.rs`
- Modify: `crates/continuitydb-memory/src/lib.rs`
- Modify: `crates/continuitydb-checkout/src/lib.rs`
- Modify: `docs/roadmap.md`
- Create: `docs/superpowers/plans/2026-05-20-system-time-lookup.md`

- [x] **Step 1: Write failing core system-time test**

Add a core test asserting that a newly created `StateCell` carries a default system-time range and that the range exposes `contains` and `from`.

- [x] **Step 2: Write failing kernel append-time tests**

Add memory and file kernel tests using `append_cell_at` to commit a cell at `2026-05-20T12:00:00Z`, then verify `CellLookup { system_at: Some(committed_at) }` returns the cell and an earlier system time does not.

- [x] **Step 3: Run tests to verify they fail**

Run:

```bash
cargo test -p continuitydb-core system_time
cargo test -p continuitydb-memory system_time
cargo test -p continuitydb-kernel system_time
```

Expected: FAIL because StateCell has no `system_time`, `CellLookup` has no `system_at`, and `StorageKernel` has no `append_cell_at`.

- [x] **Step 4: Implement core system-time support**

Add `SystemTimeRange::contains`, `SystemTimeRange::from`, `Default`, and `SystemTimeRange::open_from`.

Add `StateCell.system_time` with `#[serde(default)]` and initialize it in `StateCell::new`.

- [x] **Step 5: Implement storage append stamping and lookup**

Change `StorageKernel` so `append_cell` defaults to `append_cell_at(cell, Utc::now())` and `append_cell_at` is implemented by memory and file kernels.

Add `CellLookup.system_at: Option<DateTime<Utc>>`.

Filter cells by `cell.system_time.contains(system_at)` in memory and file kernels.

- [x] **Step 6: Update checkout test fixture**

Update `RecordingKernel` in `crates/continuitydb-checkout/src/lib.rs` to implement `append_cell_at` instead of `append_cell` if required by the trait change.

- [x] **Step 7: Run focused tests**

Run:

```bash
cargo test -p continuitydb-core system_time
cargo test -p continuitydb-memory system_time
cargo test -p continuitydb-kernel system_time
cargo test -p continuitydb-checkout
```

Expected: PASS.

- [x] **Step 8: Update roadmap**

Add a storage-kernel milestone noting first-class system transaction-time stamping and lookup.

- [x] **Step 9: Run workspace verification**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

Expected: all commands pass.

- [x] **Step 10: Commit**

```bash
git add crates/continuitydb-core/src/time.rs crates/continuitydb-core/src/cell.rs crates/continuitydb-kernel/src/lib.rs crates/continuitydb-memory/src/lib.rs crates/continuitydb-checkout/src/lib.rs docs/roadmap.md docs/superpowers/plans/2026-05-20-system-time-lookup.md
git commit -m "feat: add state cell system time"
```
