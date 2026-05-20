# Native Steward Audit API Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add feature-gated native API methods for recording and reading Steward proposal audit records through `ContinuityDb<K>`.

**Architecture:** `continuitydb-steward` gets a borrowed kernel proposal store adapter over `&mut K`. `continuitydb-api` gets an optional `steward` feature and delegates proposal recording/listing/lookup through `StoredProposalLedger<BorrowedKernelProposalStore<_>>`.

**Tech Stack:** Rust 2021, existing `continuitydb-api`, `continuitydb-steward`, `StorageKernel`, `ProposalLedgerStore`.

---

## File Structure

- Modify `crates/continuitydb-steward/src/ledger.rs`: add `BorrowedKernelProposalStore<'a, K>`.
- Modify `crates/continuitydb-steward/src/lib.rs`: re-export the borrowed store and add tests.
- Modify `crates/continuitydb-api/Cargo.toml`: add optional `continuitydb-steward` dependency and `steward` feature.
- Modify `crates/continuitydb-api/src/lib.rs`: add feature-gated error variant, imports, native Steward audit methods, and tests.
- Modify `README.md` and `docs/roadmap.md`: record the roadmap slice.

## Task 1: RED Tests

- [ ] Add Steward borrowed-store and API tests in `crates/continuitydb-steward/src/lib.rs` and `crates/continuitydb-api/src/lib.rs`.
- [ ] Add optional `continuitydb-steward` dependency and `steward` feature so the API tests can compile far enough to fail on missing methods.
- [ ] Run `cargo test -p continuitydb-api steward_proposal --features steward`.
- [ ] Expected: FAIL because `ContinuityDb` has no Steward audit methods and `BorrowedKernelProposalStore` does not exist.

## Task 2: Implementation

- [ ] Implement `BorrowedKernelProposalStore<'a, K>` with `new`, `kernel`, `kernel_mut`, and `ProposalLedgerStore` impl using the same `record_to_cell`, `record_from_cell`, and `proposal_anchor` helpers as `KernelProposalStore`.
- [ ] Re-export `BorrowedKernelProposalStore`.
- [ ] Add `ContinuityError::Steward(#[from] StewardError)` behind `#[cfg(feature = "steward")]`.
- [ ] Add `ContinuityDb<K>` methods behind `#[cfg(feature = "steward")]`:
  - `record_steward_proposal`
  - `steward_proposal_records`
  - `steward_proposal_record`
- [ ] Run targeted Steward/API tests.

## Task 3: Docs, Gate, Commit

- [ ] Update README current scope with native Steward proposal audit API.
- [ ] Add Native API and Steward roadmap milestones.
- [ ] Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

- [ ] Commit with `feat: add native steward audit api`.

## Self-Review

- Spec coverage: The plan covers the borrowed kernel store, optional API feature, native methods, docs, and verification.
- Placeholder scan: No TODO/TBD placeholders remain.
- Type consistency: Method and type names match the design.
