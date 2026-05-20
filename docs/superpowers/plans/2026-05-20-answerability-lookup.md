# Answerability Lookup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add answerability filtering to `CellLookup` so kernels can retrieve StateCells by the questions they can answer.

**Architecture:** Expose read-only answerability questions from `Answerability`. Extend `CellLookup` with an optional exact answerability question filter and apply it in both memory and file kernels.

**Tech Stack:** Rust 2021, existing `Answerability` model in `continuitydb-core`.

---

### Task 1: RED Answerability Tests

**Files:**
- Modify: `crates/continuitydb-core/src/cell.rs`
- Modify: `crates/continuitydb-kernel/src/lib.rs`
- Modify: `crates/continuitydb-memory/src/lib.rs`

- [x] **Step 1: Write failing core accessor test**

Add a test proving `Answerability::questions()` returns the normalized stored question slice.

- [x] **Step 2: Write failing file-kernel test**

Add `file_kernel_filters_by_answerability_question`. It should append two cells with different answerability questions, lookup with `answerability_question: Some("what is frontier?".to_string())`, and assert only the matching cell is returned.

- [x] **Step 3: Write failing memory-kernel test**

Add `memory_kernel_filters_by_answerability_question` with the same behavior for `MemoryKernel`.

- [x] **Step 4: Verify RED**

Run:

```bash
cargo test -p continuitydb-core answerability_questions
cargo test -p continuitydb-kernel answerability_question
cargo test -p continuitydb-memory answerability_question
```

Expected: compilation fails because `Answerability::questions` and `CellLookup::answerability_question` are not implemented.

### Task 2: Implement Answerability Filtering

**Files:**
- Modify: `crates/continuitydb-core/src/cell.rs`
- Modify: `crates/continuitydb-kernel/src/lib.rs`
- Modify: `crates/continuitydb-memory/src/lib.rs`
- Modify: `crates/continuitydb-checkout/src/lib.rs`

- [x] **Step 1: Add `Answerability::questions`**

Return `&[String]` from the private stored questions vector.

- [x] **Step 2: Extend `CellLookup`**

Add:

```rust
pub answerability_question: Option<String>,
```

- [x] **Step 3: Filter file and memory kernels**

Filter cells where any `cell.answerability.questions()` entry exactly equals the lookup question.

- [x] **Step 4: Update direct `CellLookup` initializers**

Use `..CellLookup::default()` where needed.

- [x] **Step 5: Verify GREEN**

Run:

```bash
cargo test -p continuitydb-core answerability_questions
cargo test -p continuitydb-kernel answerability_question
cargo test -p continuitydb-memory answerability_question
```

Expected: all targeted tests pass.

### Task 3: Roadmap and Verification

**Files:**
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-answerability-lookup.md`

- [x] **Step 1: Update roadmap**

Update the storage-kernel milestones to include answerability-question filtering.

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
