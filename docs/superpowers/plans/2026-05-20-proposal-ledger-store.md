# Proposal Ledger Store Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a pluggable storage-backed proposal ledger contract for Steward audit records.

**Architecture:** Keep `ProposalLedger` as the simple in-memory append-only ledger, and add a `ProposalLedgerStore` trait plus `StoredProposalLedger<S>` wrapper. This gives durable storage engines a stable append/list/get-by-id contract while preserving deterministic proposal and policy validation.

**Tech Stack:** Rust 2021, existing `StewardProposal`, `ProposalDecision`, `ProposalAuditRecord`, and `StewardError`.

---

### Task 1: RED Store Tests

**Files:**
- Modify: `crates/continuitydb-steward/src/lib.rs`

- [ ] **Step 1: Write failing tests**

Add tests proving:
- a stored ledger records proposal audit records through a pluggable store.
- stored records can be listed in insertion order and retrieved by proposal ID.
- mismatched proposal and decision IDs are rejected before storage mutation.

- [ ] **Step 2: Verify RED**

Run: `cargo test -p continuitydb-steward stored_proposal`

Expected: compilation fails because `MemoryProposalStore`, `ProposalLedgerStore`, and `StoredProposalLedger` are not exported yet.

### Task 2: Implement Store Contract

**Files:**
- Modify: `crates/continuitydb-steward/src/ledger.rs`
- Modify: `crates/continuitydb-steward/src/lib.rs`

- [ ] **Step 1: Add storage-backed ledger types**

Implement:
- `ProposalLedgerStore`
- `MemoryProposalStore`
- `StoredProposalLedger<S>`

`StoredProposalLedger::record` must validate via `ProposalAuditRecord::new` before appending to storage.

- [ ] **Step 2: Verify GREEN**

Run: `cargo test -p continuitydb-steward stored_proposal`

Expected: new stored proposal ledger tests pass.

### Task 3: Docs and Full Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [ ] **Step 1: Mark progress**

Update current scope and the Steward milestone 3 caveat to reflect the pluggable proposal ledger store contract.

- [ ] **Step 2: Verify**

Run:
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace --all-features`
- `cargo test --workspace`

Expected: all checks pass.
