# Revision Link Audit Traces Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Include native revision-link records in direct StateCell audit traces.

**Architecture:** Add `revision_links` to `AuditTrace` with empty default behavior in `continuitydb-checkout::audit`. Enrich `ContinuityDb::audit_cell` by querying source-side and target-side native revision links from the backing `StorageKernel`.

**Tech Stack:** Rust 2021, `continuitydb-checkout`, `continuitydb-api`, `continuitydb-kernel`.

---

### Task 1: RED API Tests

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`

- [x] **Step 1: Add revision-link audit tests**

Add tests named:

- `api_audit_cell_includes_native_revision_links`
- `api_audit_cell_deduplicates_self_revision_links`

The first test should create three cells, record one link where the audited cell is source and one link where it is target, then assert `trace.revision_links` preserves both records in source-then-target order.

The second test should create one cell, record a self-link, then assert `trace.revision_links` contains exactly one record.

- [x] **Step 2: Verify RED**

Run:

```bash
cargo test -p continuitydb-api api_audit_cell --all-features
```

Expected: FAIL because `AuditTrace` does not yet expose `revision_links`.

### Task 2: GREEN Implementation

**Files:**
- Modify: `crates/continuitydb-checkout/src/lib.rs`
- Modify: `crates/continuitydb-api/src/lib.rs`

- [x] **Step 1: Extend `AuditTrace`**

Add `revision_links: Vec<RevisionLinkRecord>` to `AuditTrace` and import `RevisionLinkRecord` from `continuitydb_core`.

Update `audit(cell)` to initialize `revision_links: Vec::new()`.

- [x] **Step 2: Enrich API audit traces**

Update `ContinuityDb::audit_cell` to:

- look up the target cell first, preserving `CellNotFound` behavior,
- build the base trace with `audit(&cell)`,
- set `trace.revision_links` to a helper result,
- return the enriched trace.

Add a private helper that lists source-side links first, then target-side links, and avoids duplicate records.

- [x] **Step 3: Verify GREEN**

Run:

```bash
cargo test -p continuitydb-api api_audit_cell --all-features
```

Expected: PASS.

### Task 3: Docs, Full Gate, Commit

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-revision-link-audit-traces.md`

- [x] **Step 1: Update docs**

Record revision-link-aware audit traces in README current scope, Checkout milestones, and Native API milestones.

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
git add README.md docs/roadmap.md crates/continuitydb-checkout/src/lib.rs crates/continuitydb-api/src/lib.rs docs/superpowers/specs/2026-05-20-revision-link-audit-traces-design.md docs/superpowers/plans/2026-05-20-revision-link-audit-traces.md
git commit -m "feat: include revision links in audit traces"
```
