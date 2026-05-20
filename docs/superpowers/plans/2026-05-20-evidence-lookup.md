# Evidence Lookup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add evidence-aware filtering to `CellLookup` so kernels can retrieve StateCells by evidence source and minimum evidence confidence.

**Architecture:** Expose read-only source identifiers from `SourceId`. Extend `CellLookup` with optional evidence-source and minimum-confidence filters, then apply identical semantics in file and memory kernels. A cell matches `evidence_source` when any evidence record has that source, and matches `minimum_confidence` when any evidence record has confidence greater than or equal to the requested threshold.

**Tech Stack:** Rust 2021, existing `continuitydb-core` evidence types, existing `StorageKernel` implementations.

---

### Task 1: RED Evidence Lookup Tests

**Files:**
- Modify: `crates/continuitydb-core/src/evidence.rs`
- Modify: `crates/continuitydb-core/src/lib.rs`
- Modify: `crates/continuitydb-kernel/src/lib.rs`
- Modify: `crates/continuitydb-memory/src/lib.rs`

- [x] **Step 1: Write failing source accessor test**

Add a test proving `SourceId::as_str()` returns the stored source identifier.

- [x] **Step 2: Write failing file-kernel source filter test**

Add `file_kernel_filters_by_evidence_source`. It should append two cells with distinct evidence sources, lookup with `evidence_source: Some("human".to_string())`, and assert only the matching cell is returned.

- [x] **Step 3: Write failing memory-kernel source filter test**

Add `memory_kernel_filters_by_evidence_source` with the same behavior for `MemoryKernel`.

- [x] **Step 4: Write failing file-kernel minimum-confidence filter test**

Add `file_kernel_filters_by_minimum_confidence`. It should append low-confidence and high-confidence cells, lookup with `minimum_confidence: Some(Confidence::new(0.8)?)`, and assert only the high-confidence cell is returned.

- [x] **Step 5: Write failing memory-kernel minimum-confidence filter test**

Add `memory_kernel_filters_by_minimum_confidence` with the same behavior for `MemoryKernel`.

- [x] **Step 6: Verify RED**

Run:

```bash
cargo test -p continuitydb-core source_id_as_str
cargo test -p continuitydb-kernel evidence_source
cargo test -p continuitydb-memory evidence_source
cargo test -p continuitydb-kernel minimum_confidence
cargo test -p continuitydb-memory minimum_confidence
```

Expected: compilation fails because `SourceId::as_str`, `CellLookup::evidence_source`, and `CellLookup::minimum_confidence` are not implemented.

### Task 2: Implement Evidence Filtering

**Files:**
- Modify: `crates/continuitydb-core/src/evidence.rs`
- Modify: `crates/continuitydb-kernel/src/lib.rs`
- Modify: `crates/continuitydb-memory/src/lib.rs`
- Modify: direct `CellLookup` initializers only if they do not use `..CellLookup::default()`

- [x] **Step 1: Add `SourceId::as_str`**

Return `&str` from the private stored source identifier.

- [x] **Step 2: Extend `CellLookup`**

Add:

```rust
pub evidence_source: Option<String>,
pub minimum_confidence: Option<Confidence>,
```

- [x] **Step 3: Filter file and memory kernels by evidence source**

Filter cells where any `cell.evidence` entry has `evidence.source.as_str() == source`.

- [x] **Step 4: Filter file and memory kernels by minimum confidence**

Filter cells where any `cell.evidence` entry has `evidence.confidence.value() >= minimum_confidence.value()`.

- [x] **Step 5: Update direct `CellLookup` initializers**

Run `rg -n "CellLookup \\{" crates` and ensure every direct initializer either sets the new fields intentionally or uses `..CellLookup::default()`.

- [x] **Step 6: Verify GREEN**

Run:

```bash
cargo test -p continuitydb-core source_id_as_str
cargo test -p continuitydb-kernel evidence_source
cargo test -p continuitydb-memory evidence_source
cargo test -p continuitydb-kernel minimum_confidence
cargo test -p continuitydb-memory minimum_confidence
```

Expected: all targeted tests pass.

### Task 3: Roadmap and Verification

**Files:**
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-evidence-lookup.md`

- [x] **Step 1: Update roadmap**

Update the storage-kernel milestones to include evidence-source and minimum-confidence filtering.

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
