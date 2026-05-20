# File Kernel Parent Directories Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `FileKernel::open` create missing parent directories for nested embedded database paths.

**Architecture:** Keep `FileKernel` append-only JSONL. Before opening the backing file, create the parent directory tree when the path has a parent component.

**Tech Stack:** Rust 2021 standard filesystem APIs.

---

### Task 1: RED Nested-Path Test

**Files:**
- Modify: `crates/continuitydb-kernel/src/lib.rs`

- [x] **Step 1: Write failing test**

Add `file_kernel_open_creates_missing_parent_directories` to the kernel tests. It should:
- Build a temp path like `<temp>/continuitydb-file-kernel-nested-<id>/db/cells.jsonl`.
- Assert the parent directory does not exist before open.
- Open `FileKernel`.
- Assert the parent directory and backing file now exist.
- Remove the temporary root directory.

- [x] **Step 2: Verify RED**

Run:

```bash
cargo test -p continuitydb-kernel file_kernel_open_creates_missing_parent_directories
```

Expected: test fails because `FileKernel::open` does not create missing parent directories.

### Task 2: Implement Parent Directory Creation

**Files:**
- Modify: `crates/continuitydb-kernel/src/lib.rs`

- [x] **Step 1: Update `FileKernel::open`**

Before `OpenOptions::open`, call `std::fs::create_dir_all(parent)` when `path.parent()` exists and is non-empty. Map errors to `KernelError::StoreIo`.

- [x] **Step 2: Verify GREEN**

Run:

```bash
cargo test -p continuitydb-kernel file_kernel_open_creates_missing_parent_directories
```

Expected: test passes.

### Task 3: Roadmap and Verification

**Files:**
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-file-kernel-parent-dirs.md`

- [x] **Step 1: Update roadmap**

Update storage kernel milestone 3 to note that `FileKernel` creates nested database directories.

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
