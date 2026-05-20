# Checkout Constraint Pushdown Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Push checkout answerability, evidence-source, and minimum-confidence constraints into `CellLookup` so continuity slices use the storage kernel's first-class semantic filters.

**Architecture:** Extend `CheckoutRequest` with optional `answerability_question` and `evidence_source` fields. Pass those fields plus `minimum_confidence` into `CellLookup` during checkout, while keeping the existing deterministic confidence retain as a defensive invariant for kernels that may be implemented incorrectly.

**Tech Stack:** Rust 2021, existing `continuitydb-checkout`, `continuitydb-kernel`, and `continuitydb-memory` crates.

---

### Task 1: RED Checkout Pushdown Tests

**Files:**
- Modify: `crates/continuitydb-checkout/src/lib.rs`

- [x] **Step 1: Write failing lookup pushdown test**

Add `checkout_pushes_semantic_constraints_to_kernel`. Use a test `RecordingKernel` that captures the incoming `CellLookup` and returns an empty cell set. The test should call `checkout` with `answerability_question`, `evidence_source`, and `minimum_confidence`, then assert the captured lookup contains those values.

- [x] **Step 2: Write failing answerability behavior test**

Add `checkout_filters_by_answerability_question`. Use `MemoryKernel`, append two cells with different answerability questions, request `"what is frontier?"`, and assert only the matching cell is returned.

- [x] **Step 3: Write failing evidence-source behavior test**

Add `checkout_filters_by_evidence_source`. Use `MemoryKernel`, append two cells with different evidence sources, request `"human"`, and assert only the matching cell is returned.

- [x] **Step 4: Verify RED**

Run:

```bash
cargo test -p continuitydb-checkout checkout_pushes_semantic_constraints_to_kernel
cargo test -p continuitydb-checkout checkout_filters_by_answerability_question
cargo test -p continuitydb-checkout checkout_filters_by_evidence_source
```

Expected: compilation fails because `CheckoutRequest::answerability_question` and `CheckoutRequest::evidence_source` are not implemented.

### Task 2: Implement Checkout Constraint Pushdown

**Files:**
- Modify: `crates/continuitydb-checkout/src/lib.rs`

- [x] **Step 1: Extend `CheckoutRequest`**

Add:

```rust
pub answerability_question: Option<String>,
pub evidence_source: Option<String>,
```

- [x] **Step 2: Pass constraints into `CellLookup`**

Set:

```rust
answerability_question: request.answerability_question,
evidence_source: request.evidence_source,
minimum_confidence: Some(request.minimum_confidence),
```

Keep the existing confidence retain to preserve checkout's deterministic inclusion invariant.

- [x] **Step 3: Update all `CheckoutRequest` initializers**

Run `rg -n "CheckoutRequest \\{" crates` and add the new optional fields where direct initializers exist.

- [x] **Step 4: Verify GREEN**

Run:

```bash
cargo test -p continuitydb-checkout checkout_pushes_semantic_constraints_to_kernel
cargo test -p continuitydb-checkout checkout_filters_by_answerability_question
cargo test -p continuitydb-checkout checkout_filters_by_evidence_source
```

Expected: all targeted tests pass.

### Task 3: Roadmap and Verification

**Files:**
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-checkout-constraint-pushdown.md`

- [x] **Step 1: Update roadmap**

Add a checkout/materialization milestone noting semantic constraint pushdown into the storage kernel.

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
