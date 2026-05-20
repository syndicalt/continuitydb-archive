# Native API Revision Link Records Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Expose storage-native revision-link records through `ContinuityDb<K>` and add a native Steward `LinkRevision` record application path.

**Architecture:** Add thin native API methods over `StorageKernel::append_revision_link` and `list_revision_links`, with API-level endpoint validation. Add a Steward-specific method that converts accepted `LinkRevision` audit records into native `RevisionLinkRecord` values.

**Tech Stack:** Rust 2021, `continuitydb-api`, `continuitydb-kernel`, `continuitydb-core`, optional `steward` feature tests.

---

### Task 1: RED API Tests

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`

- [x] **Step 1: Add native API revision-link test**

Add a test proving `record_revision_link_at` appends and `list_revision_links` filters the record.

- [x] **Step 2: Add Steward native record tests**

Add tests for accepted, rejected, missing-source, and missing-target `apply_accepted_link_revision_record_proposal_at`.

- [x] **Step 3: Verify RED**

Run:

```bash
cargo test -p continuitydb-api revision_link_record_api --all-features
```

Expected: FAIL because the new API methods do not exist yet.

### Task 2: GREEN API Implementation

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`

- [x] **Step 1: Add imports**

Import `RevisionLinkKind`, `RevisionLinkRecord`, and `RevisionLinkLookup` where needed.

- [x] **Step 2: Implement native revision-link APIs**

Implement endpoint-validating `record_revision_link_at` and pass-through `list_revision_links`.

- [x] **Step 3: Implement native Steward record application**

Implement `apply_accepted_link_revision_record_proposal_at` using the native API method.

- [x] **Step 4: Verify GREEN**

Run:

```bash
cargo test -p continuitydb-api revision_link_record_api --all-features
```

Expected: PASS.

### Task 3: Docs, Full Gate, Commit

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-native-api-revision-link-records.md`

- [x] **Step 1: Update docs**

Record native API revision-link records in README current scope and Native API milestones.

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

Commit with `feat: expose native revision link records in api`.
