# Checkout System Time Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let checkout materialize continuity slices as of both valid time and system transaction time.

**Architecture:** Extend `CheckoutRequest` with `system_at: Option<DateTime<Utc>>` and pass it directly into `CellLookup.system_at`. Keep checkout ranking and packing unchanged; storage kernels already own deterministic transaction-time filtering.

**Tech Stack:** Rust 2021, `chrono`, `continuitydb-checkout`, `continuitydb-kernel`, `continuitydb-memory`.

---

### Task 1: System-Time Checkout Pushdown

**Files:**
- Modify: `crates/continuitydb-checkout/src/lib.rs`
- Modify: `docs/roadmap.md`
- Create: `docs/superpowers/plans/2026-05-20-checkout-system-time.md`

- [x] **Step 1: Write failing pushdown test**

Add a checkout test using `RecordingKernel` that sets `CheckoutRequest.system_at` and asserts captured `CellLookup.system_at` matches the request.

- [x] **Step 2: Write failing behavior test**

Add a checkout test with a `MemoryKernel` containing one cell committed at `2026-05-20T12:00:00Z`. Assert checkout with `system_at` before commit returns no cells and checkout at commit time returns the committed cell.

- [x] **Step 3: Verify RED**

Run:

```bash
cargo test -p continuitydb-checkout checkout_pushes_system_time_to_kernel
cargo test -p continuitydb-checkout checkout_filters_by_system_time
```

Expected: FAIL because `CheckoutRequest` has no `system_at` field and checkout does not push system transaction time into `CellLookup`.

- [x] **Step 4: Implement request field and pushdown**

Add:

```rust
pub system_at: Option<DateTime<Utc>>,
```

to `CheckoutRequest`, and pass:

```rust
system_at: request.system_at,
```

inside the `CellLookup` built by `checkout`.

- [x] **Step 5: Update existing test requests**

Add `system_at: None` to existing `CheckoutRequest` literals.

- [x] **Step 6: Run focused tests**

Run:

```bash
cargo test -p continuitydb-checkout checkout_pushes_system_time_to_kernel
cargo test -p continuitydb-checkout checkout_filters_by_system_time
cargo test -p continuitydb-checkout
```

Expected: PASS.

- [x] **Step 7: Update roadmap**

Add a checkout milestone noting system transaction-time checkout constraints are pushed into storage lookup.

- [x] **Step 8: Run workspace verification**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

Expected: all commands pass.

- [x] **Step 9: Commit**

```bash
git add crates/continuitydb-checkout/src/lib.rs docs/roadmap.md docs/superpowers/plans/2026-05-20-checkout-system-time.md
git commit -m "feat: checkout by system time"
```
