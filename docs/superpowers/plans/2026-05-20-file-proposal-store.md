# File Proposal Store Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a durable file-backed proposal audit store.

**Architecture:** `FileProposalStore` implements the existing `ProposalLedgerStore` trait using append-only JSONL. Each audit record is serialized as one line, and reads rebuild ordered records from the file. This keeps the durable adapter simple and inspectable while preserving proposal/decision validation in `StoredProposalLedger`.

**Tech Stack:** Rust 2021, standard library filesystem APIs, `serde`, `serde_json`, existing `ProposalAuditRecord` and `StoredProposalLedger`.

---

### Task 1: RED File Store Tests

**Files:**
- Modify: `crates/continuitydb-steward/src/lib.rs`

- [ ] **Step 1: Write failing tests**

Add tests proving:
- records appended to `FileProposalStore` are available after reopening the store at the same path.
- `record_by_id` works after reopening.
- invalid JSONL content returns a deterministic Steward storage error.

- [ ] **Step 2: Verify RED**

Run: `cargo test -p continuitydb-steward file_proposal`

Expected: compilation fails because `FileProposalStore` is not exported yet.

### Task 2: Implement File Store

**Files:**
- Modify: `crates/continuitydb-steward/Cargo.toml`
- Modify: `crates/continuitydb-steward/src/error.rs`
- Modify: `crates/continuitydb-steward/src/ledger.rs`
- Modify: `crates/continuitydb-steward/src/lib.rs`

- [ ] **Step 1: Add serde support and errors**

Make `serde_json` available to the Steward crate outside `local-model`, derive serde for `ProposalAuditRecord`, and add deterministic file-store error variants.

- [ ] **Step 2: Add `FileProposalStore`**

Implement append, list, and get-by-id against JSONL.

- [ ] **Step 3: Verify GREEN**

Run: `cargo test -p continuitydb-steward file_proposal`

Expected: file-backed proposal store tests pass.

### Task 3: Docs and Full Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [ ] **Step 1: Mark progress**

Update current scope and milestone 3 to reflect the JSONL file-backed store.

- [ ] **Step 2: Verify**

Run:
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace --all-features`
- `cargo test --workspace`

Expected: all checks pass.
