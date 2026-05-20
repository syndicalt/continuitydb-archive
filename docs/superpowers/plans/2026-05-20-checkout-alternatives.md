# Checkout Alternatives Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add deterministic alternative metadata to `CheckoutSlice` for candidate StateCells excluded by checkout packing.

**Architecture:** Track candidates that satisfy lookup and confidence constraints but cannot fit inside the token budget. Represent each excluded candidate as serializable metadata with its cell ID, exclusion reason, citation locators, and max confidence so callers can explain what was omitted without mutating storage semantics.

**Tech Stack:** Rust 2021, existing `continuitydb-checkout` types and deterministic checkout packing.

---

### Task 1: RED Checkout Alternatives Tests

**Files:**
- Modify: `crates/continuitydb-checkout/src/lib.rs`

- [x] **Step 1: Write failing token-budget alternative test**

Add `checkout_records_token_budget_alternatives`. It should append two high-confidence cells where only the first fits in the token budget, run checkout, then assert:

- `slice.cells` contains the selected first cell.
- `slice.alternatives` contains one entry for the omitted second cell.
- The alternative reason is `CheckoutAlternativeReason::TokenBudgetExceeded`.
- The alternative includes the omitted cell's citation and max confidence.

- [x] **Step 2: Verify RED**

Run:

```bash
cargo test -p continuitydb-checkout checkout_records_token_budget_alternatives
```

Expected: compilation fails because `CheckoutSlice::alternatives`, `CheckoutAlternative`, and `CheckoutAlternativeReason` are not implemented.

### Task 2: Implement Checkout Alternatives

**Files:**
- Modify: `crates/continuitydb-checkout/src/lib.rs`

- [x] **Step 1: Add alternative reason enum**

Add:

```rust
pub enum CheckoutAlternativeReason {
    TokenBudgetExceeded,
}
```

- [x] **Step 2: Add alternative metadata struct**

Add:

```rust
pub struct CheckoutAlternative {
    pub cell_id: StateCellId,
    pub reason: CheckoutAlternativeReason,
    pub citations: Vec<String>,
    pub max_confidence: Confidence,
}
```

- [x] **Step 3: Extend `CheckoutSlice`**

Add:

```rust
pub alternatives: Vec<CheckoutAlternative>,
```

- [x] **Step 4: Populate token-budget alternatives during packing**

When a candidate would exceed `request.token_budget`, do not select it. Push a `CheckoutAlternative` with `TokenBudgetExceeded`, citation locators, and max confidence.

- [x] **Step 5: Verify GREEN**

Run:

```bash
cargo test -p continuitydb-checkout checkout_records_token_budget_alternatives
cargo test -p continuitydb-checkout
```

Expected: checkout tests pass.

### Task 3: Roadmap and Verification

**Files:**
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-checkout-alternatives.md`

- [x] **Step 1: Update roadmap**

Add a checkout milestone for deterministic token-budget alternatives.

- [x] **Step 2: Mark this plan complete**

Check off completed steps in this plan before commit.

- [x] **Step 3: Verify**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

Expected: all checks pass.
