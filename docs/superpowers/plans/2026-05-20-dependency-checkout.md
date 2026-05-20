# Dependency-Aware Checkout Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add dependency-aware checkout constraints so materialized continuity slices can target cells connected to a specific causal or dependency reference.

**Architecture:** Extend `CheckoutRequest` with optional dependency target and dependency kind filters, then push them into `CellLookup` alongside the existing semantic filters. Preserve deterministic checkout ranking and token packing after storage lookup.

**Tech Stack:** Rust 2021, `continuitydb-core`, `continuitydb-kernel`, `continuitydb-checkout`, `continuitydb-memory`.

---

### Task 1: Dependency Checkout Filters

**Files:**
- Modify: `crates/continuitydb-checkout/src/lib.rs`
- Modify: `docs/roadmap.md`
- Create: `docs/superpowers/plans/2026-05-20-dependency-checkout.md`

- [x] **Step 1: Write failing tests**

Add one test that verifies checkout pushes dependency constraints into `CellLookup` using `RecordingKernel`, and one test that verifies `MemoryKernel` checkout returns only cells with a matching dependency target and kind.

Use fields:

```rust
dependency_target: Some(target),
dependency_kind: Some(CellDependencyKind::DependsOn),
```

- [x] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p continuitydb-checkout dependency
```

Expected: FAIL because `CheckoutRequest` does not expose dependency filters yet.

- [x] **Step 3: Implement request fields and pushdown**

Add to `CheckoutRequest`:

```rust
pub dependency_target: Option<StateCellId>,
pub dependency_kind: Option<CellDependencyKind>,
```

Pass both through to `CellLookup` in `checkout`.

- [x] **Step 4: Run focused tests**

Run:

```bash
cargo test -p continuitydb-checkout dependency
cargo test -p continuitydb-checkout
```

Expected: PASS.

- [x] **Step 5: Update roadmap**

Add a checkout milestone for dependency-aware checkout constraints.

- [x] **Step 6: Run workspace verification**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

Expected: all commands pass.

- [x] **Step 7: Commit**

```bash
git add crates/continuitydb-checkout/src/lib.rs docs/roadmap.md docs/superpowers/plans/2026-05-20-dependency-checkout.md
git commit -m "feat: checkout by dependencies"
```
