# Revision Link Import Validation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reject duplicate native revision-link records during commit export batch import before mutation.

**Architecture:** Extend `ContinuityDb::validate_commit_export_batch` with duplicate detection for incoming revision links and pre-existing target revision links. Keep storage-kernel behavior unchanged and rely on native import validation to preserve replay semantics.

**Tech Stack:** Rust 2021, `continuitydb-api`, `RevisionLinkRecord`, `RevisionLinkLookup`.

---

### Task 1: RED Import Validation Tests

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`

- [x] **Step 1: Add duplicate-link batch test**

Add `revision_link_import_rejects_duplicate_links_without_mutation`. Build a source export batch with two cells and one revision link, duplicate the link inside the batch, import into an empty target, and assert:

- import returns `ContinuityError::InvalidCommitExport`,
- no commit slices are visible in target,
- no revision links are visible in target.

- [x] **Step 2: Add existing-target-link test**

Add `revision_link_import_rejects_existing_target_link_without_mutation`. Seed target with the same cells and link through one import, then attempt to import a link-only or duplicate-link batch that would append the same revision link again, and assert:

- import returns `ContinuityError::InvalidCommitExport`,
- target still has one revision link.

- [x] **Step 3: Verify RED**

Run:

```bash
cargo test -p continuitydb-api revision_link_import_rejects --all-features
```

Expected: FAIL because duplicate revision-link records are currently accepted.

### Task 2: GREEN Implementation

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`

- [x] **Step 1: Detect duplicate links in batch**

In `validate_commit_export_batch`, track `batch.revision_links` with a small vector or set and reject repeated records.

- [x] **Step 2: Detect existing target links**

List current target revision links once and reject imported records already present in the target.

- [x] **Step 3: Verify GREEN**

Run:

```bash
cargo test -p continuitydb-api revision_link_import_rejects --all-features
```

Expected: PASS.

### Task 3: Docs, Full Gate, Commit

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-revision-link-import-validation.md`

- [x] **Step 1: Update docs**

Record duplicate-safe revision-link import validation in README current scope and Native API milestones.

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
git add README.md docs/roadmap.md crates/continuitydb-api/src/lib.rs docs/superpowers/specs/2026-05-20-revision-link-import-validation-design.md docs/superpowers/plans/2026-05-20-revision-link-import-validation.md
git commit -m "feat: validate revision link import duplicates"
```
