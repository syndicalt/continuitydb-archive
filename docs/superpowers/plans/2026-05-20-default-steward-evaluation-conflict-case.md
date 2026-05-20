# Default Steward Evaluation Conflict Case Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add conflict-classification coverage to the default local Steward model benchmark suite.

**Architecture:** Extend `default_steward_evaluation_suite()` with a deterministic second case using stable `StateCellId::from_u128` IDs and the existing `LinkRevision` proposal machinery. Update tests and CLI fixture output to satisfy both default cases.

**Tech Stack:** Rust 2021, `continuitydb-steward`, `continuitydb-cli`, feature-gated `local-model`.

---

### Task 1: Default Conflict Evaluation Case

**Files:**
- Modify: `crates/continuitydb-steward/src/local_model.rs`
- Modify: `crates/continuitydb-steward/src/lib.rs`
- Modify: `crates/continuitydb-cli/tests/cli.rs`
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [x] **Step 1: Write failing test expectations**

Update `default_steward_evaluation_suite_scores_valid_verification_proposal` to emit both the existing verification proposal and a `link_revision` proposal:

```rust
let source = StateCellId::from_u128(1);
let target = StateCellId::from_u128(2);
```

Assert:

```rust
assert_eq!(report.case_reports().len(), 2);
assert!(report.case_reports()[1].passed());
assert_eq!(report.case_reports()[1].name(), "conflict classification");
```

- [x] **Step 2: Verify RED**

Run:

```bash
cargo test -p continuitydb-steward default_steward_evaluation_suite_scores_valid_verification_proposal --features local-model
```

Expected: test failure because the default suite still has one case.

- [x] **Step 3: Implement the second default case**

In `default_steward_evaluation_suite()`, add:

```rust
let conflict_source = StateCellId::from_u128(1);
let conflict_target = StateCellId::from_u128(2);
```

and append a `conflict classification` case requiring `RevisionLinkKind::ConflictsWith`.

- [x] **Step 4: Update CLI fixture**

Update the `cli_benchmark_local_model_records_baseline` shell fixture to emit both proposals, and update assertions from one case to two cases:

```rust
assert_eq!(json["passed_cases"].as_u64(), Some(2));
assert_eq!(json["total_cases"].as_u64(), Some(2));
assert_eq!(
    json["evaluation"]["case_reports"][1]["name"].as_str(),
    Some("conflict classification")
);
```

- [x] **Step 5: Verify GREEN**

Run:

```bash
cargo test -p continuitydb-steward default_steward_evaluation_suite_scores_valid_verification_proposal --features local-model
cargo test -p continuitydb-cli cli_benchmark_local_model_records_baseline --features local-model
```

Expected: both tests pass.

- [x] **Step 6: Update docs**

Add README current-scope bullet:

```markdown
- Default local model conflict-classification evaluation case.
```

Add Steward milestone:

```markdown
33. Add default local-model conflict-classification evaluation. Expanded the fixed Steward benchmark suite with a deterministic `ConflictsWith` revision-link case so local model baselines test classification behavior beyond thin-evidence verification.
```

- [x] **Step 7: Run full gate**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

Expected: all commands pass.

- [x] **Step 8: Commit**

Run:

```bash
git add README.md docs/roadmap.md docs/superpowers/specs/2026-05-20-default-steward-evaluation-conflict-case-design.md docs/superpowers/plans/2026-05-20-default-steward-evaluation-conflict-case.md crates/continuitydb-steward/src/local_model.rs crates/continuitydb-steward/src/lib.rs crates/continuitydb-cli/tests/cli.rs
git commit -m "feat: add default steward conflict evaluation"
```
