# Revision Link Commit Export Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Preserve native revision-link records through commit export, import, direct copy, and CLI backup/restore.

**Architecture:** Add a `revision_links` vector to `CommitExportBatch`. Export source-owned revision links for cells in the selected commit page, validate that imported link endpoints are present or already stored, append cells before links, and let file JSON helpers plus CLI commands reuse the same native path.

**Tech Stack:** Rust 2021, `continuitydb-api`, `continuitydb-cli`, `RevisionLinkLookup`, versioned JSON commit export envelopes.

---

### Task 1: RED Revision-Link Export Tests

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`
- Modify: `crates/continuitydb-cli/tests/cli.rs`

- [x] **Step 1: Add failing native API tests**

Add tests that assert:

- `export_commits` includes a native revision link whose source cell is in the exported page,
- encode/decode preserves `CommitExportBatch.revision_links`,
- importing a batch restores the revision link,
- `copy_commits_from` restores the revision link.

- [x] **Step 2: Add failing CLI backup test**

Extend `cli_exports_and_imports_commit_backup` or add a sibling test so a source file store with a native revision link exports to JSON, imports into a target, and the target lists the same revision link.

- [x] **Step 3: Verify RED**

Run:

```bash
cargo test -p continuitydb-api revision_link_commit_export --all-features
cargo test -p continuitydb-cli cli_exports_and_imports_commit_backup --all-features
```

Expected: FAIL because `CommitExportBatch` has no `revision_links` field and export/import ignores native revision-link records.

### Task 2: GREEN Implementation

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`

- [x] **Step 1: Extend batch type**

Add `pub revision_links: Vec<RevisionLinkRecord>` to `CommitExportBatch`.

- [x] **Step 2: Export source-owned links**

Update `export_commits` to collect selected cell IDs in commit/cell order, query `RevisionLinkLookup { source: Some(cell_id), ..Default::default() }`, deduplicate, and store the resulting records in the batch.

- [x] **Step 3: Validate revision-link endpoints**

Update `validate_commit_export_batch` so every exported link has both endpoints either already present in the target kernel or in the imported batch's cell IDs.

- [x] **Step 4: Import revision links**

Update `import_commit_batch_with_summary` to append all exported revision links after appending all cells.

- [x] **Step 5: Update empty/malformed test constructors**

Add `revision_links: Vec::new()` to direct `CommitExportBatch` literals in tests.

- [x] **Step 6: Verify GREEN**

Run:

```bash
cargo test -p continuitydb-api revision_link_commit_export --all-features
cargo test -p continuitydb-cli cli_exports_and_imports_commit_backup --all-features
```

Expected: PASS.

### Task 3: Docs, Full Gate, Commit

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-revision-link-commit-export.md`

- [x] **Step 1: Update docs**

Record revision-link-aware commit export/import/copy in README current scope and roadmap native API/CLI milestones.

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
git add README.md docs/roadmap.md crates/continuitydb-api/src/lib.rs crates/continuitydb-cli/tests/cli.rs docs/superpowers/specs/2026-05-20-revision-link-commit-export-design.md docs/superpowers/plans/2026-05-20-revision-link-commit-export.md
git commit -m "feat: preserve revision links in commit export"
```
