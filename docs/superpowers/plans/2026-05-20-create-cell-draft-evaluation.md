# Create Cell Draft Evaluation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a default local Steward model evaluation case that tests `CreateCellDraft` proposals from new evidence.

**Architecture:** Extend only the fixed local-model evaluation suite and its tests. Existing CLI benchmark, prompt artifact, stability, failure-report, and suite-export paths should expose the new case through existing suite-driven behavior.

**Tech Stack:** Rust, serde_json, existing `continuitydb-steward` local-model evaluation tests, existing `continuitydb-cli` local-model tests.

---

### Task 1: RED Tests

**Files:**
- Modify: `crates/continuitydb-steward/src/lib.rs`
- Modify: `crates/continuitydb-cli/tests/cli.rs`

- [x] Update `default_steward_evaluation_suite_scores_valid_verification_proposal` to expect nine case reports and include a valid `create_cell_draft` proposal with anchor `project:continuitydb:benchmark-result`.
- [x] Update `default_steward_evaluation_suite_exposes_case_contracts` to expect nine cases and assert case index 6 is `new evidence draft creation`.
- [x] Update CLI complete-response fixtures to include the same draft proposal and expect nine passing cases.
- [x] Update CLI failure report assertions so a fixture that only passes the first case reports eight failed cases.
- [x] Update CLI prompt artifact assertions to expect nine artifacts and inspect the draft-creation prompt.
- [x] Update CLI evaluation-suite export assertions to expect nine cases and inspect the draft-creation case.
- [x] Run focused tests and verify RED:

```bash
cargo test -p continuitydb-steward default_steward_evaluation_suite --features local-model
cargo test -p continuitydb-cli cli_local_model_evaluation_suite_outputs_case_contracts --features local-model
```

### Task 2: Implementation

**Files:**
- Modify: `crates/continuitydb-steward/src/local_model.rs`

- [x] Insert a `StewardEvaluationCase` named `new evidence draft creation` after the targeted verification case.
- [x] The case task is `Draft a StateCell from new benchmark evidence.`
- [x] The case evidence locator is `continuitydb://evaluation/new-benchmark-evidence`.
- [x] The case expected action is `StewardAction::CreateCellDraft { anchors: vec![SemanticAnchor::new("project:continuitydb:benchmark-result")], payload_text: "ContinuityDB local Steward benchmark produced a new result requiring review.".to_string() }`.
- [x] Require the draft evidence citation and rationale term `draft`.
- [x] Forbid rationale term `committed`.
- [x] Run focused tests and verify GREEN:

```bash
cargo test -p continuitydb-steward default_steward_evaluation_suite --features local-model
cargo test -p continuitydb-cli cli_local_model_evaluation_suite_outputs_case_contracts --features local-model
```

### Task 3: Documentation and Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-create-cell-draft-evaluation.md`

- [x] Add README current-scope bullet for the default local-model create-cell-draft evaluation case.
- [x] Add roadmap Steward milestone for the default local-model create-cell-draft evaluation case.
- [x] Run full verification:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

- [x] Commit with message:

```bash
git commit -m "feat: add draft creation steward evaluation"
```
