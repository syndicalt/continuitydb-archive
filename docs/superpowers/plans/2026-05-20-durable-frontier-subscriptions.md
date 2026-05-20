# Durable Frontier Subscriptions Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add durable frontier/watch subscriptions so watched StateCells and their relevant signals can survive process restarts.

**Architecture:** Extend `continuitydb-steward` frontier integration with subscription records, subscription IDs, matching semantics, and a pluggable append-only store contract. Mirror the existing proposal ledger storage approach with in-memory and JSONL file-backed adapters while keeping subscription decisions deterministic and model-free.

**Tech Stack:** Rust 2021, `chrono`, `serde`, `serde_json`, `uuid`, existing `FrontierWatchSignal`, `FrontierWatchEvent`, `StateCellId`, and `StewardError`.

---

### Task 1: RED Subscription Tests

**Files:**
- Modify: `crates/continuitydb-steward/src/lib.rs`

- [ ] **Step 1: Write failing tests**

Add tests proving:
- A `FrontierSubscription` rejects empty signal lists and empty citations.
- A subscription matches only events for the same cell and subscribed signal.
- `MemoryFrontierSubscriptionStore` lists subscriptions in insertion order and gets a subscription by ID.
- `FileFrontierSubscriptionStore` persists subscriptions across reopen and rejects corrupt JSONL.

- [ ] **Step 2: Verify RED**

Run: `cargo test -p continuitydb-steward frontier_subscription`

Expected: compilation fails because subscription types and stores are not implemented or exported.

### Task 2: Implement Durable Subscription Layer

**Files:**
- Modify: `crates/continuitydb-steward/src/frontier.rs`
- Modify: `crates/continuitydb-steward/src/error.rs`
- Modify: `crates/continuitydb-steward/src/lib.rs`

- [ ] **Step 1: Add public frontier subscription types**

Implement:
- `FrontierSubscriptionId`
- `FrontierSubscription`
- `FrontierSubscriptionStore`
- `MemoryFrontierSubscriptionStore`
- `FileFrontierSubscriptionStore`

`FrontierSubscription::new` should trim and validate citation text, reject empty signal lists, and expose getters plus `matches_event(&FrontierWatchEvent)`.

- [ ] **Step 2: Add store error variants**

Add `EmptyFrontierSubscription`, `FrontierSubscriptionStoreIo`, and `FrontierSubscriptionStoreCorrupt` variants to `StewardError`.

- [ ] **Step 3: Verify GREEN**

Run: `cargo test -p continuitydb-steward frontier_subscription`

Expected: all subscription tests pass.

### Task 3: Docs and Full Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [ ] **Step 1: Mark roadmap progress**

Update current scope and Steward milestone 7 to say durable frontier subscriptions exist, while richer subscription execution remains future work if still needed.

- [ ] **Step 2: Verify**

Run:
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace --all-features`
- `cargo test --workspace`
- `git diff --check`

Expected: all checks pass.
