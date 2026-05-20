# Checkout Metadata Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Enrich `CheckoutSlice` with deterministic citation, uncertainty, and frontier metadata for selected StateCells.

**Architecture:** Reuse the existing `AuditTrace` shape for selected-cell citations. Add small serializable metadata structs for selected-cell uncertainty and frontier recommendations. Populate metadata during checkout from the final selected cells so the slice remains deterministic and kernel-independent.

**Tech Stack:** Rust 2021, existing `continuitydb-checkout` and `continuitydb-core` types.

---

### Task 1: RED Checkout Metadata Tests

**Files:**
- Modify: `crates/continuitydb-checkout/src/lib.rs`

- [x] **Step 1: Write failing metadata test**

Add `checkout_slice_includes_citations_uncertainty_and_frontier_metadata`. It should append an active selected cell and a frontier selected cell, run checkout, then assert:

- `slice.audit_traces` contains one trace per selected cell with citation locators.
- `slice.uncertainty` contains one entry per selected cell with each cell's max confidence.
- `slice.frontier_recommendations` contains the frontier cell ID and supporting citation.

- [x] **Step 2: Verify RED**

Run:

```bash
cargo test -p continuitydb-checkout checkout_slice_includes_citations_uncertainty_and_frontier_metadata
```

Expected: compilation fails because `CheckoutSlice::audit_traces`, `CheckoutSlice::uncertainty`, `CheckoutSlice::frontier_recommendations`, `UncertaintyEntry`, and `FrontierRecommendation` are not implemented.

### Task 2: Implement Checkout Metadata

**Files:**
- Modify: `crates/continuitydb-checkout/src/lib.rs`

- [x] **Step 1: Import `ActivationState`**

Add `ActivationState` to the existing `continuitydb_core` import list.

- [x] **Step 2: Add metadata structs**

Add serializable public structs:

```rust
pub struct UncertaintyEntry {
    pub cell_id: StateCellId,
    pub max_confidence: Confidence,
}

pub struct FrontierRecommendation {
    pub cell_id: StateCellId,
    pub citations: Vec<String>,
}
```

- [x] **Step 3: Extend `CheckoutSlice`**

Add:

```rust
pub audit_traces: Vec<AuditTrace>,
pub uncertainty: Vec<UncertaintyEntry>,
pub frontier_recommendations: Vec<FrontierRecommendation>,
```

- [x] **Step 4: Populate metadata during checkout**

After the final `cells` vector is selected:

- `audit_traces`: `cells.iter().map(audit).collect()`
- `uncertainty`: one entry per selected cell using its max evidence confidence.
- `frontier_recommendations`: one entry for every selected cell with `ActivationState::Frontier`, using the same citation locators as `audit`.

- [x] **Step 5: Update affected tests**

Update existing checkout tests only where they construct or assert `CheckoutSlice` fields directly. They should continue asserting `cells` and `total_tokens`, and the new metadata test should assert the new fields.

- [x] **Step 6: Verify GREEN**

Run:

```bash
cargo test -p continuitydb-checkout checkout_slice_includes_citations_uncertainty_and_frontier_metadata
cargo test -p continuitydb-checkout
```

Expected: checkout tests pass.

### Task 3: Roadmap and Verification

**Files:**
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-checkout-metadata.md`

- [x] **Step 1: Update roadmap**

Add a checkout milestone for selected-cell citations, uncertainty metadata, and frontier recommendations.

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
