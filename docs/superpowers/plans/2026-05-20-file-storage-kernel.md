# File Storage Kernel Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a durable append-only file-backed `StorageKernel` implementation for embedded ContinuityDB use.

**Architecture:** Implement `FileKernel` in `continuitydb-kernel` as a JSONL append log of serialized `StateCell` records. Opening the kernel creates the file if absent, appends reject duplicate cell IDs by scanning existing records, and lookups perform deterministic full-scan filtering with the existing `CellLookup` constraints.

**Tech Stack:** Rust 2021, `serde_json`, existing `StateCell` serde support.

---

### Task 1: RED Durable Kernel Tests

**Files:**
- Modify: `crates/continuitydb-kernel/src/lib.rs`

- [x] **Step 1: Write failing tests**

Add tests for:
- `file_kernel_persists_cells_across_reopen`
- `file_kernel_rejects_duplicate_after_reopen`
- `file_kernel_rejects_corrupt_jsonl`

The persistence test should open a temp JSONL file, append a sample cell, reopen `FileKernel`, and assert lookup by semantic anchor returns the cell.

The duplicate test should append a cell, reopen the same file, and assert appending the same cell returns `KernelError::DuplicateCell`.

The corrupt test should write invalid JSONL to a temp file, open `FileKernel`, call `lookup_cells`, and assert `KernelError::StoreCorrupt`.

- [x] **Step 2: Verify RED**

Run:

```bash
cargo test -p continuitydb-kernel file_kernel
```

Expected: compilation fails because `FileKernel` and new error variants are not implemented.

### Task 2: Implement FileKernel

**Files:**
- Modify: `crates/continuitydb-kernel/Cargo.toml`
- Modify: `crates/continuitydb-kernel/src/lib.rs`

- [x] **Step 1: Add dependency**

Add `serde_json.workspace = true` to `continuitydb-kernel` dependencies.

- [x] **Step 2: Add error variants**

Add:
- `KernelError::StoreIo`
- `KernelError::StoreCorrupt`

- [x] **Step 3: Add `FileKernel`**

Add `FileKernel { path: PathBuf }` with:
- `open(path) -> Result<Self, KernelError>`
- `path(&self) -> &Path`
- private `read_cells() -> Result<Vec<StateCell>, KernelError>`

- [x] **Step 4: Implement `StorageKernel`**

`append_cell` should:
- call `read_cells()`
- reject duplicate `StateCellId`
- append one JSON line with trailing newline

`lookup_cells` should reuse the same filter semantics as `MemoryKernel`.

- [x] **Step 5: Verify GREEN**

Run:

```bash
cargo test -p continuitydb-kernel file_kernel
```

Expected: file kernel tests pass.

### Task 3: Roadmap and Verification

**Files:**
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-file-storage-kernel.md`

- [x] **Step 1: Update roadmap**

Add a storage-kernel milestone note that a JSONL append-only `FileKernel` now exists as the first durable embedded kernel, while indexed production storage remains future work.

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
