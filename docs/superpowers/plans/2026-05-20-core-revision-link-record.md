# Core Revision Link Record Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a core semantic `RevisionLinkRecord` as the stable foundation for storage-native revision links.

**Architecture:** Define `RevisionLinkKind` and `RevisionLinkRecord` in `continuitydb-core`, export them from core, and re-export `RevisionLinkKind` from `continuitydb-revision` for compatibility. Do not change storage persistence in this slice.

**Tech Stack:** Rust 2021, serde, chrono, existing core and revision crates.

---

### Task 1: RED Core Tests

**Files:**
- Modify: `crates/continuitydb-core/src/lib.rs`
- Modify: `crates/continuitydb-revision/src/lib.rs`

- [x] **Step 1: Add failing core test**

Add a core test proving `RevisionLinkRecord::new` preserves source, target, kind, and recorded time and serializes with serde.

- [x] **Step 2: Add failing compatibility test**

Add a revision crate test proving `continuitydb_revision::RevisionLinkKind` is still importable and usable.

- [x] **Step 3: Verify RED**

Run: `cargo test -p continuitydb-core revision_link_record -p continuitydb-revision revision_link_kind_reexport`

Expected: FAIL because the core record does not exist yet.

### Task 2: GREEN Core Type

**Files:**
- Modify: `crates/continuitydb-core/src/cell.rs`
- Modify: `crates/continuitydb-core/src/lib.rs`
- Modify: `crates/continuitydb-revision/src/lib.rs`

- [x] **Step 1: Implement core `RevisionLinkKind` and `RevisionLinkRecord`**

Add serializable, comparable types in `cell.rs` with a `new` constructor.

- [x] **Step 2: Re-export from core and revision**

Export both core types from `continuitydb-core`; re-export `RevisionLinkKind` from `continuitydb-revision`.

- [x] **Step 3: Verify GREEN**

Run:

```bash
cargo test -p continuitydb-core revision_link_record
cargo test -p continuitydb-revision revision_link_kind_reexport
```

Expected: PASS.

### Task 3: Docs, Full Gate, Commit

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-core-revision-link-record.md`

- [x] **Step 1: Update docs**

Record the core native revision-link record in README current scope and Dependency and Causality milestones.

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

Commit with `feat: add core revision link records`.
