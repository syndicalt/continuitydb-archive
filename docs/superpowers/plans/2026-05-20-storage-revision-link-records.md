# Storage Revision Link Records Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add storage-kernel-native append-only revision-link records.

**Architecture:** Extend the storage trait with a `RevisionLinkLookup`, `append_revision_link`, and `list_revision_links`. Implement in memory and file kernels, with file JSONL records using the same checksum and compaction path as cells and commits.

**Tech Stack:** Rust 2021, serde JSONL file kernel, existing memory/file kernel tests.

---

### Task 1: RED Kernel Tests

**Files:**
- Modify: `crates/continuitydb-memory/src/lib.rs`
- Modify: `crates/continuitydb-kernel/src/lib.rs`

- [x] **Step 1: Add memory kernel failing test**

Add a `revision_link_storage_memory_kernel_appends_and_filters_links` test using `append_revision_link` and `list_revision_links`.

- [x] **Step 2: Add file kernel failing tests**

Add tests for persistence across reopen, lookup filtering, and compaction preservation.

- [x] **Step 3: Verify RED**

Run:

```bash
cargo test revision_link_storage
```

Expected: FAIL because the storage trait and lookup type do not exist yet.

### Task 2: GREEN Storage Implementation

**Files:**
- Modify: `crates/continuitydb-kernel/src/lib.rs`
- Modify: `crates/continuitydb-memory/src/lib.rs`

- [x] **Step 1: Add trait and lookup types**

Add `RevisionLinkLookup`, `append_revision_link`, and `list_revision_links`.

- [x] **Step 2: Implement memory kernel support**

Store `RevisionLinkRecord` values in append order and apply conjunctive lookup filters.

- [x] **Step 3: Implement file kernel support**

Add revision-link JSONL records, checksum validation, log loading, in-memory indexing/listing, append, and compaction preservation.

- [x] **Step 4: Verify GREEN**

Run:

```bash
cargo test revision_link_storage
```

Expected: PASS.

### Task 3: Docs, Full Gate, Commit

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-storage-revision-link-records.md`

- [x] **Step 1: Update docs**

Record storage-native revision-link records in README current scope and Storage Kernel milestones.

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

Commit with `feat: store native revision link records`.
