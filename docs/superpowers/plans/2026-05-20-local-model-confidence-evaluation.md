# Local Model Confidence Evaluation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a default local Steward model evaluation case that tests valid confidence-adjustment proposals.

**Architecture:** Extend only the fixed evaluation suite and its tests. The existing suite-driven CLI paths should automatically expose the new case in benchmark JSON, prompt artifacts, failure reports, stability reports, and evaluation-suite exports.

**Tech Stack:** Rust, serde_json, existing `continuitydb-steward` local-model evaluation tests, existing `continuitydb-cli` local-model tests.

---

### Task 1: RED Tests

**Files:**
- Modify: `crates/continuitydb-steward/src/lib.rs`
- Modify: `crates/continuitydb-cli/tests/cli.rs`

- [x] Update `default_steward_evaluation_suite_scores_valid_verification_proposal` to expect seven case reports and include a valid `adjust_confidence` proposal:

```json
{
  "action": {
    "type": "adjust_confidence",
    "cell_id": "00000000-0000-0000-0000-000000000006",
    "proposed_confidence": 0.42
  },
  "rationale": "The cited evidence lowers confidence in the stale deployment status.",
  "citations": ["continuitydb://evaluation/confidence-evidence"]
}
```

- [x] Update `default_steward_evaluation_suite_exposes_case_contracts` to expect seven cases and assert case index 4 is `confidence adjustment` with `StewardAction::AdjustConfidence`, citation `continuitydb://evaluation/confidence-evidence`, rationale term `confidence`, and forbidden term `fully trusted`.
- [x] Update CLI complete-response fixtures to include the same `adjust_confidence` proposal and expect seven passing cases.
- [x] Update CLI evaluation-suite export assertions to expect seven cases and inspect the confidence case.
- [x] Update CLI prompt artifact assertions to inspect the confidence prompt.
- [x] Run focused tests and verify RED:

```bash
cargo test -p continuitydb-steward default_steward_evaluation_suite --features local-model
cargo test -p continuitydb-cli cli_local_model_evaluation_suite_outputs_case_contracts --features local-model
```

### Task 2: Implementation

**Files:**
- Modify: `crates/continuitydb-steward/src/local_model.rs`

- [x] Add `confidence_cell = StateCellId::from_u128(6)` in `default_steward_evaluation_suite`.
- [x] Insert a `StewardEvaluationCase` named `confidence adjustment` after the unsupported-claim case.
- [x] The case task is `Adjust confidence for stale deployment status evidence.`
- [x] The case evidence locator is `continuitydb://evaluation/confidence-evidence`.
- [x] The case expected action is `StewardAction::AdjustConfidence { cell_id: confidence_cell, proposed_confidence: 0.42 }`.
- [x] Require the confidence citation and rationale term `confidence`.
- [x] Forbid rationale term `fully trusted`.
- [x] Run focused tests and verify GREEN:

```bash
cargo test -p continuitydb-steward default_steward_evaluation_suite --features local-model
cargo test -p continuitydb-cli cli_local_model_evaluation_suite_outputs_case_contracts --features local-model
```

### Task 3: Documentation and Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-local-model-confidence-evaluation.md`

- [x] Add README current-scope bullet for the default local-model confidence-adjustment evaluation case.
- [x] Add roadmap Steward milestone for the default local-model confidence-adjustment evaluation case.
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
git commit -m "feat: add confidence steward evaluation"
```
