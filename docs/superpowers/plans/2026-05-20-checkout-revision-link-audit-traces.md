# Checkout Revision Link Audit Traces Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Include native revision-link records in checkout-selected audit traces.

**Architecture:** Preserve `audit(&StateCell)` as a pure baseline and enrich audit traces inside `checkout(kernel, request)` because that function has storage-kernel access. Reuse source-first, target-second, deduplicated revision-link lookup semantics.

**Tech Stack:** Rust 2021, `continuitydb-checkout`, `continuitydb-kernel`, `continuitydb-memory`.

---

### Task 1: RED Checkout Tests

**Files:**
- Modify: `crates/continuitydb-checkout/src/lib.rs`

- [x] **Step 1: Add checkout revision-link audit tests**

Add tests named:

- `checkout_audit_traces_include_native_revision_links`
- `checkout_audit_traces_deduplicate_self_revision_links`

The first test should append three cells, append one native revision link where the selected cell is source and one where it is target, run checkout, and assert the selected cell audit trace contains both records in source-then-target order.

The second test should append one selected cell, append a self-link, run checkout, and assert the selected audit trace contains exactly one record.

- [x] **Step 2: Verify RED**

Run:

```bash
cargo test -p continuitydb-checkout checkout_audit_traces --all-features
```

Expected: FAIL because checkout currently leaves `AuditTrace.revision_links` empty.

### Task 2: GREEN Implementation

**Files:**
- Modify: `crates/continuitydb-checkout/src/lib.rs`

- [x] **Step 1: Import revision lookup type**

Import `RevisionLinkLookup` from `continuitydb_kernel`.

- [x] **Step 2: Add helper**

Add a private helper that lists source-side links, then target-side links, deduplicating records:

```rust
fn revision_links_for_cell<K: StorageKernel>(
    kernel: &K,
    cell_id: StateCellId,
) -> Result<Vec<RevisionLinkRecord>, CheckoutError>
```

- [x] **Step 3: Enrich checkout audit traces**

Replace `cells.iter().map(audit).collect()` with a fallible collection that sets `trace.revision_links` from the helper.

- [x] **Step 4: Verify GREEN**

Run:

```bash
cargo test -p continuitydb-checkout checkout_audit_traces --all-features
```

Expected: PASS.

### Task 3: Docs, Full Gate, Commit

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-checkout-revision-link-audit-traces.md`

- [x] **Step 1: Update docs**

Record revision-link-aware checkout audit traces in README current scope and Checkout milestones.

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
git add README.md docs/roadmap.md crates/continuitydb-checkout/src/lib.rs docs/superpowers/specs/2026-05-20-checkout-revision-link-audit-traces-design.md docs/superpowers/plans/2026-05-20-checkout-revision-link-audit-traces.md
git commit -m "feat: include revision links in checkout audit traces"
```
