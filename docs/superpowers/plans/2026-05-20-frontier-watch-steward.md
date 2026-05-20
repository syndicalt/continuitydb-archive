# Frontier Watch Steward Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add frontier/watch integration so the Steward can propose refresh and verification work from deterministic frontier signals.

**Architecture:** The integration lives in `continuitydb-steward` as a deterministic frontier watch evaluator. Watch events are evidence-backed inputs; the evaluator emits ordinary `StewardProposal` values, preserving the existing rule that Stewards propose and deterministic policy decides what becomes committed truth.

**Tech Stack:** Rust 2021, `chrono`, `continuitydb-core::StateCellId`, existing `StewardAction`, `StewardProposal`, and `ProposalPolicy`.

---

### Task 1: RED Frontier Watch Tests

**Files:**
- Modify: `crates/continuitydb-steward/src/lib.rs`

- [ ] **Step 1: Write failing tests**

Add tests proving:
- stale frontier evidence emits a `RequestVerification` proposal with the watched cell and citation.
- high-impact uncertain evidence emits a `MarkFrontier` proposal.
- benign events emit no proposals.
- emitted proposals pass strict policy and can be recorded in the ledger.

- [ ] **Step 2: Verify RED**

Run: `cargo test -p continuitydb-steward frontier_watch`

Expected: compilation fails because frontier watch types are not exported yet.

### Task 2: Implement Frontier Watch Module

**Files:**
- Create: `crates/continuitydb-steward/src/frontier.rs`
- Modify: `crates/continuitydb-steward/src/lib.rs`

- [ ] **Step 1: Add types and evaluator**

Implement:
- `FrontierWatchEvent`
- `FrontierWatchSignal`
- `FrontierSteward`

The evaluator must emit:
- `RequestVerification` for stale evidence events.
- `MarkFrontier` for high-impact uncertainty events.
- no proposal for benign events.

- [ ] **Step 2: Verify GREEN**

Run: `cargo test -p continuitydb-steward frontier_watch`

Expected: new frontier watch tests pass.

### Task 3: Docs and Workspace Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [ ] **Step 1: Mark roadmap progress**

Update current scope and milestone 7 to reflect deterministic frontier/watch proposal integration.

- [ ] **Step 2: Verify**

Run:
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace --all-features`
- `cargo test --workspace`

Expected: all checks pass.
