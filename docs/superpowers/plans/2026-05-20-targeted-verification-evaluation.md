# Targeted Verification Evaluation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a default local Steward model evaluation case that tests targeted verification requests against a specific StateCell.

**Architecture:** Extend only the fixed local-model evaluation suite and tests. Existing CLI benchmark, prompt artifact, stability, failure-report, and suite-export paths should expose the new case through the existing suite-driven plumbing.

**Tech Stack:** Rust, serde_json, existing `continuitydb-steward` local-model evaluation tests, existing `continuitydb-cli` local-model tests.

---

### Task 1: RED Tests

**Files:**
- Modify: `crates/continuitydb-steward/src/lib.rs`
- Modify: `crates/continuitydb-cli/tests/cli.rs`

- [x] Update `default_steward_evaluation_suite_scores_valid_verification_proposal` to expect eight case reports and include a valid targeted `request_verification` proposal for `00000000-0000-0000-0000-000000000007`.
- [x] Update `default_steward_evaluation_suite_exposes_case_contracts` to expect eight cases and assert case index 5 is `targeted verification request`.
- [x] Update CLI complete-response fixtures to include the same targeted verification proposal and expect eight passing cases.
- [x] Update CLI failure report assertions so a fixture that only passes the first case reports seven failed cases.
- [x] Update CLI prompt artifact assertions to expect eight artifacts and inspect the targeted verification prompt.
- [x] Update CLI evaluation-suite export assertions to expect eight cases and inspect the targeted verification case.
- [x] Run focused tests and verify RED:

```bash
cargo test -p continuitydb-steward default_steward_evaluation_suite --features local-model
cargo test -p continuitydb-cli cli_local_model_evaluation_suite_outputs_case_contracts --features local-model
```

### Task 2: Implementation

**Files:**
- Modify: `crates/continuitydb-steward/src/local_model.rs`

- [x] Add `targeted_verification_cell = StateCellId::from_u128(7)` in `default_steward_evaluation_suite`.
- [x] Insert a `StewardEvaluationCase` named `targeted verification request` after the confidence case.
- [x] The case task is `Request verification for a stale high-impact frontier cell.`
- [x] The case evidence locator is `continuitydb://evaluation/targeted-verification-evidence`.
- [x] The case expected action is `StewardAction::RequestVerification { cell_id: Some(targeted_verification_cell), request: "Refresh the stale high-impact frontier signal.".to_string() }`.
- [x] Require the targeted verification citation and rationale term `refresh`.
- [x] Forbid rationale term `no target`.
- [x] Run focused tests and verify GREEN:

```bash
cargo test -p continuitydb-steward default_steward_evaluation_suite --features local-model
cargo test -p continuitydb-cli cli_local_model_evaluation_suite_outputs_case_contracts --features local-model
```

### Task 3: Documentation and Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-targeted-verification-evaluation.md`

- [x] Add README current-scope bullet for the default local-model targeted-verification evaluation case.
- [x] Add roadmap Steward milestone for the default local-model targeted-verification evaluation case.
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
git commit -m "feat: add targeted verification steward evaluation"
```
