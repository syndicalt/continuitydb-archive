# Duplicate Revision-Link Storage Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reject exact duplicate native revision-link records at the storage-kernel boundary and surface the invariant through the native API.

**Architecture:** Add a typed duplicate revision-link kernel error, enforce it before mutation in memory and file kernels, and reject duplicate durable file records during index rebuild. The native API does not need special duplicate handling because it already propagates kernel errors.

**Tech Stack:** Rust 2021, `continuitydb-core`, `continuitydb-kernel`, `continuitydb-memory`, `continuitydb-api`, JSONL file kernel.

---

### Task 1: RED Tests

**Files:**
- Modify: `crates/continuitydb-memory/src/lib.rs`
- Modify: `crates/continuitydb-kernel/src/lib.rs`
- Modify: `crates/continuitydb-api/src/lib.rs`

- [x] **Step 1: Add memory-kernel duplicate test**

Add `revision_link_storage_memory_kernel_rejects_duplicate_links` near the existing memory revision-link tests. It appends the same `RevisionLinkRecord` twice, expects `KernelError::DuplicateRevisionLink`, and verifies only one link remains visible.

- [x] **Step 2: Add file-kernel duplicate append test**

Add `revision_link_storage_file_kernel_rejects_duplicate_links_without_writing_record` near the existing file revision-link tests. It appends once, verifies the second append returns `KernelError::DuplicateRevisionLink`, verifies the visible links contain one record, and verifies the JSONL file still has only header plus one revision-link record.

- [x] **Step 3: Add file-kernel duplicate reopen test**

Add `revision_link_storage_file_kernel_rejects_duplicate_links_on_reopen`. Write a checksum-free header plus two identical `revision_link` envelope records, then assert `FileKernel::open` returns `KernelError::DuplicateRevisionLink`.

- [x] **Step 4: Add native API duplicate test**

Add `revision_link_record_api_rejects_duplicate_record` near `revision_link_record_api_records_and_lists_links`. It creates source and target cells, records one link, records the same link again with the same timestamp, expects `ContinuityError::Kernel(KernelError::DuplicateRevisionLink)`, and verifies one visible link remains.

- [x] **Step 5: Run RED focused tests**

Run:

```bash
cargo test -p continuitydb-memory revision_link_storage_memory_kernel_rejects_duplicate_links --all-features
cargo test -p continuitydb-kernel revision_link_storage_file_kernel_rejects_duplicate_links --all-features
cargo test -p continuitydb-api revision_link_record_api_rejects_duplicate_record --all-features
```

Expected: FAIL before implementation because `KernelError::DuplicateRevisionLink` does not exist.

### Task 2: GREEN Implementation

**Files:**
- Modify: `crates/continuitydb-kernel/src/lib.rs`
- Modify: `crates/continuitydb-memory/src/lib.rs`

- [x] **Step 1: Add `KernelError::DuplicateRevisionLink`**

Add a new error variant after `DuplicateCommit`:

```rust
/// A duplicate immutable revision-link record was appended.
#[error("revision link already exists")]
DuplicateRevisionLink,
```

- [x] **Step 2: Enforce duplicate rejection in memory kernel**

Before pushing in `MemoryKernel::append_revision_link`, return `KernelError::DuplicateRevisionLink` when `self.revision_links.contains(&revision_link)`.

- [x] **Step 3: Enforce duplicate rejection in file-kernel index rebuild**

Change `FileKernelIndex::insert_revision_link` to return `Result<(), KernelError>`. It should check `self.revision_links.contains(&revision_link)` before mutating indexes and return `KernelError::DuplicateRevisionLink` on duplicates.

- [x] **Step 4: Enforce duplicate rejection before file append writes**

In `FileKernel::append_revision_link`, check `self.index.revision_links.contains(&revision_link)` before encoding/opening/writing. After durable write, call `self.index.insert_revision_link(revision_link)?`.

- [x] **Step 5: Run GREEN focused tests**

Run the same focused tests from Task 1. Expected: PASS.

### Task 3: Docs and Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-duplicate-revision-link-storage.md`

- [x] **Step 1: Update README scope**

Add a current-scope bullet for duplicate-safe revision-link storage and API append validation.

- [x] **Step 2: Update roadmap**

Add Storage Kernel milestone 30 for duplicate revision-link append rejection and Native API milestone 41 for duplicate revision-link record append errors.

- [x] **Step 3: Run full verification gate**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

Expected: PASS.

- [x] **Step 4: Mark plan complete**

Check off all completed boxes in this plan.

- [x] **Step 5: Commit**

Commit with:

```bash
git add README.md docs/roadmap.md docs/superpowers/specs/2026-05-20-duplicate-revision-link-storage-design.md docs/superpowers/plans/2026-05-20-duplicate-revision-link-storage.md crates/continuitydb-kernel/src/lib.rs crates/continuitydb-memory/src/lib.rs crates/continuitydb-api/src/lib.rs
git commit -m "feat: reject duplicate revision link appends"
```
