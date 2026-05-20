# File Kernel Revision Link Indexes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add derived in-process indexes for file-kernel revision-link source, target, and kind lookups.

**Architecture:** Keep JSONL records as the durable source of truth and rebuild revision-link indexes into `FileKernelIndex` on open. Maintain indexes only after a successful durable append, then route `list_revision_links` through the best matching index while preserving append-order results and predicate filtering.

**Tech Stack:** Rust 2021, `continuitydb-kernel`, JSONL `FileKernel`, `RevisionLinkLookup`.

---

### Task 1: RED Revision-Link Index Tests

**Files:**
- Modify: `crates/continuitydb-kernel/src/lib.rs`

- [x] **Step 1: Add file-kernel revision-link index tests**

Add tests named:

- `file_kernel_rebuilds_revision_link_indexes`
- `file_kernel_updates_revision_link_indexes_after_append`

The rebuild test should append matching and unrelated revision-link records, reopen the file kernel, inspect private `FileKernelIndex` maps from the unit test module, and assert source, target, kind, source-kind, target-kind, source-target, and source-target-kind indexes point to the expected records in append order.

The append test should append one revision-link record and assert the same maps are populated before reopening.

- [x] **Step 2: Verify RED**

Run:

```bash
cargo test -p continuitydb-kernel revision_link_indexes --all-features
```

Expected: FAIL because `FileKernelIndex` has no revision-link index maps yet.

### Task 2: GREEN Implementation

**Files:**
- Modify: `crates/continuitydb-kernel/src/lib.rs`

- [x] **Step 1: Add revision-link index fields**

Add derived position maps to `FileKernelIndex` for source, target, kind, source-kind, target-kind, source-target, and source-target-kind lookup.

- [x] **Step 2: Centralize revision-link insertion**

Add a private `insert_revision_link` helper that pushes the record into `revision_links` and updates all derived maps with the new vector position.

- [x] **Step 3: Rebuild and append through the helper**

Update `FileKernelIndex::rebuild` and `FileKernel::append_revision_link` to use `insert_revision_link` only after durable records have been accepted.

- [x] **Step 4: Route filtered listing through indexes**

Update `FileKernelIndex::list_revision_links` to choose the most specific available position list from `RevisionLinkLookup`, then retain predicate filtering and append-order results.

- [x] **Step 5: Verify GREEN**

Run:

```bash
cargo test -p continuitydb-kernel revision_link_indexes --all-features
cargo test -p continuitydb-kernel revision_link_storage_file_kernel --all-features
```

Expected: PASS.

### Task 3: Docs, Full Gate, Commit

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-file-kernel-revision-link-indexes.md`

- [x] **Step 1: Update docs**

Record file-kernel revision-link lookup indexes in README current scope and Storage Kernel milestones.

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

Commit with:

```bash
git add README.md docs/roadmap.md crates/continuitydb-kernel/src/lib.rs docs/superpowers/specs/2026-05-20-file-kernel-revision-link-indexes-design.md docs/superpowers/plans/2026-05-20-file-kernel-revision-link-indexes.md
git commit -m "feat: index file revision link lookups"
```
