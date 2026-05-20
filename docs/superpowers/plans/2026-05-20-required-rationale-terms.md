# Required Rationale Terms Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add deterministic evaluation support for requiring rationale terms, including explicit uncertainty language when evidence is insufficient.

**Architecture:** Extend `StewardEvaluationCase` with required rationale terms alongside the existing forbidden rationale terms. Evaluation should normalize rationale text case-insensitively, fail when no proposal rationale contains a required term, and keep existing policy/citation/action checks unchanged.

**Tech Stack:** Rust 2021, existing `local-model` evaluation harness in `continuitydb-steward`.

---

### Task 1: RED Required Rationale Tests

**Files:**
- Modify: `crates/continuitydb-steward/src/lib.rs`

- [x] **Step 1: Write failing tests**

Add feature-gated tests proving:
- A case passes when the model rationale includes a required uncertainty term.
- A case fails with `MissingRationaleTerm` when the rationale omits the required uncertainty term.

- [x] **Step 2: Verify RED**

Run: `cargo test -p continuitydb-steward --features local-model rationale_term`

Expected: compilation fails because `require_rationale_term` and `MissingRationaleTerm` are not implemented.

### Task 2: Implement Required Rationale Terms

**Files:**
- Modify: `crates/continuitydb-steward/src/local_model.rs`

- [x] **Step 1: Add required term state and builder**

Add:
- `required_rationale_terms: Vec<String>` to `StewardEvaluationCase`
- `require_rationale_term(self, term) -> Self`

- [x] **Step 2: Add failure variant and evaluator check**

Add:
- `StewardEvaluationFailure::MissingRationaleTerm { term: String }`
- Case-insensitive check against emitted proposal rationales.

- [x] **Step 3: Verify GREEN**

Run: `cargo test -p continuitydb-steward --features local-model rationale_term`

Expected: required rationale term tests pass.

### Task 3: Docs and Full Verification

**Files:**
- Modify: `docs/roadmap.md`

- [x] **Step 1: Mark acceptance coverage progress**

Update the Steward Acceptance Tests section to note that explicit uncertainty can now be scored through required rationale terms.

- [x] **Step 2: Verify**

Run:
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace --all-features`
- `cargo test --workspace`
- `git diff --check`

Expected: all checks pass.
