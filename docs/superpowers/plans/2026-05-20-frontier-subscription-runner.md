# Frontier Subscription Runner Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add deterministic execution for durable frontier subscriptions so watch events only produce Steward proposals when a stored subscription matches them.

**Architecture:** Extend `continuitydb-steward` with a `FrontierSubscriptionRunner<S>` that composes a `FrontierSubscriptionStore` and `FrontierSteward`. The runner loads durable subscriptions, filters incoming `FrontierWatchEvent`s through `FrontierSubscription::matches_event`, deduplicates per input event, and delegates proposal creation to the existing deterministic Steward.

**Tech Stack:** Rust 2021, existing `FrontierSubscriptionStore`, `FrontierSubscription`, `FrontierWatchEvent`, `FrontierSteward`, and `StewardProposal` APIs.

---

### Task 1: RED Runner Tests

**Files:**
- Modify: `crates/continuitydb-steward/src/lib.rs`

- [ ] **Step 1: Write failing tests**

Add tests proving:
- A runner emits a verification proposal only for a stale event matching a stored stale-evidence subscription.
- A runner suppresses unsubscribed events, including events for the wrong cell or wrong signal.
- Multiple matching subscriptions for the same event still produce one Steward proposal for that event.

- [ ] **Step 2: Verify RED**

Run: `cargo test -p continuitydb-steward frontier_subscription_runner`

Expected: compilation fails because `FrontierSubscriptionRunner` is not implemented or exported.

### Task 2: Implement Runner

**Files:**
- Modify: `crates/continuitydb-steward/src/frontier.rs`
- Modify: `crates/continuitydb-steward/src/lib.rs`

- [ ] **Step 1: Add public runner type**

Implement:
- `FrontierSubscriptionRunner<S>`

The runner should own a `FrontierSteward` and subscription store, expose `new`, `store`, `store_mut`, `into_store`, and `propose_subscribed`.

- [ ] **Step 2: Verify GREEN**

Run: `cargo test -p continuitydb-steward frontier_subscription_runner`

Expected: all runner tests pass.

### Task 3: Docs and Full Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [ ] **Step 1: Mark roadmap progress**

Update current scope and Steward milestone 7 to say durable subscriptions can now execute deterministically against incoming watch events.

- [ ] **Step 2: Verify**

Run:
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace --all-features`
- `cargo test --workspace`
- `git diff --check`

Expected: all checks pass.
