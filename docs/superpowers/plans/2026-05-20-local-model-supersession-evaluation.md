# Local Model Supersession Evaluation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a default local Steward model evaluation case that distinguishes supersession from conflict.

**Architecture:** Extend only the fixed evaluation suite and its tests. The existing suite-driven CLI paths should automatically expose the new case in benchmark JSON, prompt artifacts, failure reports, stability reports, and evaluation-suite exports.

**Tech Stack:** Rust, serde_json, existing `continuitydb-steward` local-model evaluation tests, existing `continuitydb-cli` local-model tests.

---

### Task 1: RED Tests

**Files:**
- Modify: `crates/continuitydb-steward/src/lib.rs`
- Modify: `crates/continuitydb-cli/tests/cli.rs`

- [x] Update `default_steward_evaluation_suite_scores_valid_verification_proposal` to expect six case reports and include a valid `supersedes` proposal:

```json
{
  "action": {
    "type": "link_revision",
    "source": "00000000-0000-0000-0000-000000000004",
    "kind": "supersedes",
    "target": "00000000-0000-0000-0000-000000000005"
  },
  "rationale": "The newer evidence supersedes the older status without contradicting it.",
  "citations": ["continuitydb://evaluation/supersession-evidence"]
}
```

- [x] Update `default_steward_evaluation_suite_exposes_case_contracts` to expect six cases and assert case index 2 is `supersession classification` with `RevisionLinkKind::Supersedes`, required citation `continuitydb://evaluation/supersession-evidence`, required rationale term `supersedes`, and forbidden term `conflicts with`.
- [x] Update CLI complete-response fixtures to include the same `supersedes` proposal and expect six passing cases.
- [x] Update CLI evaluation-suite export assertions to expect six cases and inspect the supersession case.
- [x] Update CLI prompt artifact assertions to inspect the supersession prompt.
- [x] Run focused tests and verify RED:

```bash
cargo test -p continuitydb-steward default_steward_evaluation_suite --features local-model
cargo test -p continuitydb-cli cli_local_model_evaluation_suite_outputs_case_contracts --features local-model
```

### Task 2: Implementation

**Files:**
- Modify: `crates/continuitydb-steward/src/local_model.rs`

- [x] Add `supersession_source = StateCellId::from_u128(4)` and `supersession_target = StateCellId::from_u128(5)` in `default_steward_evaluation_suite`.
- [x] Insert a `StewardEvaluationCase` named `supersession classification` after the conflict case.
- [x] The case task is `Classify whether newer release evidence supersedes the older status.`
- [x] The case evidence locator is `continuitydb://evaluation/supersession-evidence`.
- [x] The case expected action is `StewardAction::LinkRevision { source: supersession_source, kind: RevisionLinkKind::Supersedes, target: supersession_target }`.
- [x] Require the supersession citation and rationale term `supersedes`.
- [x] Forbid rationale term `conflicts with`.
- [x] Run focused tests and verify GREEN:

```bash
cargo test -p continuitydb-steward default_steward_evaluation_suite --features local-model
cargo test -p continuitydb-cli cli_local_model_evaluation_suite_outputs_case_contracts --features local-model
```

### Task 3: Documentation and Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-local-model-supersession-evaluation.md`

- [x] Add README current-scope bullet for the default local-model supersession evaluation case.
- [x] Add roadmap Steward milestone for the default local-model supersession evaluation case.
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
git commit -m "feat: add supersession steward evaluation"
```
