# Kernel Proposal Store Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `StorageKernel`-backed proposal audit store so Steward proposal decisions can persist through ContinuityDB's embedded storage substrate.

**Architecture:** Implement `KernelProposalStore<K>` in `continuitydb-steward` as an adapter from `ProposalLedgerStore` to any `continuitydb_kernel::StorageKernel`. Each `ProposalAuditRecord` is encoded into a `StateCell` with stable semantic anchors for all proposal audits and the specific proposal ID, preserving the existing append-only record contract while letting embedded kernels own storage.

**Tech Stack:** Rust 2021, existing `ProposalLedgerStore`, `ProposalAuditRecord`, `StoredProposalLedger`, `StorageKernel`, `StateCell`, and serde JSON payloads.

---

### Task 1: RED Kernel Store Tests

**Files:**
- Modify: `crates/continuitydb-steward/Cargo.toml`
- Modify: `crates/continuitydb-steward/src/lib.rs`

- [ ] **Step 1: Write failing tests**

Add tests proving:
- `KernelProposalStore<MemoryKernel>` records proposal audit records through `StoredProposalLedger` and lists them in insertion order.
- `KernelProposalStore<MemoryKernel>` gets a record by proposal ID.
- `KernelProposalStore<MemoryKernel>` preserves the backing kernel through `into_kernel`.

- [ ] **Step 2: Verify RED**

Run: `cargo test -p continuitydb-steward kernel_proposal_store`

Expected: compilation fails because `KernelProposalStore` is not implemented or exported.

### Task 2: Implement Kernel Adapter

**Files:**
- Modify: `crates/continuitydb-steward/Cargo.toml`
- Modify: `crates/continuitydb-steward/src/ledger.rs`
- Modify: `crates/continuitydb-steward/src/lib.rs`

- [ ] **Step 1: Add dependencies**

Add `continuitydb-kernel` to dependencies and `continuitydb-memory` to dev-dependencies for tests.

- [ ] **Step 2: Add adapter type**

Implement:
- `KernelProposalStore<K>`
- `KernelProposalStore::new(kernel) -> Self`
- `kernel(&self) -> &K`
- `kernel_mut(&mut self) -> &mut K`
- `into_kernel(self) -> K`

The adapter should implement `ProposalLedgerStore` by encoding records into JSON `StateCell` payloads and querying cells through semantic anchors.

- [ ] **Step 3: Verify GREEN**

Run: `cargo test -p continuitydb-steward kernel_proposal_store`

Expected: all kernel proposal store tests pass.

### Task 3: Docs and Full Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [ ] **Step 1: Mark roadmap progress**

Update current scope and Steward milestone 3 to say a `StorageKernel` proposal audit store adapter exists, while specialized LatticeDB or other production engine adapters remain future work.

- [ ] **Step 2: Verify**

Run:
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace --all-features`
- `cargo test --workspace`
- `git diff --check`

Expected: all checks pass.
