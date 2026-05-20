# Default Policy Rejection Evaluation Case Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a default local Steward benchmark case for deterministic policy rejection avoidance.

**Architecture:** Extend the existing fixed `default_steward_evaluation_suite` with one case that expects `RequestVerification` instead of a policy-invalid answerability label. Update suite introspection and CLI fixtures to reflect the new default contract.

**Tech Stack:** Rust, existing `continuitydb-steward` local-model feature, existing `continuitydb-cli` local-model CLI tests.

---

### Task 1: RED Tests

**Files:**
- Modify: `crates/continuitydb-steward/src/lib.rs`
- Modify: `crates/continuitydb-cli/tests/cli.rs`

- [x] Update `default_steward_evaluation_suite_exposes_case_contracts` to expect five cases and verify the fifth case contract.
- [x] Update CLI local-model suite export test to expect five cases and verify the fifth case JSON.
- [x] Update CLI prompt artifact test to expect five prompt artifacts and verify the fifth prompt.
- [x] Run focused tests and verify RED:

```bash
cargo test -p continuitydb-steward default_steward_evaluation_suite_exposes_case_contracts --features local-model
cargo test -p continuitydb-cli cli_local_model_evaluation_suite_outputs_case_contracts --features local-model
cargo test -p continuitydb-cli cli_benchmark_local_model_prompt_dir_writes_prompt_artifacts --features local-model
```

### Task 2: Implementation

**Files:**
- Modify: `crates/continuitydb-steward/src/local_model.rs`
- Modify: `crates/continuitydb-cli/tests/cli.rs`

- [x] Add the `policy rejection avoidance` case to `default_steward_evaluation_suite`.
- [x] Update passing local-model CLI fixtures to emit the fifth expected `RequestVerification` proposal.
- [x] Update stability fixtures that need all default cases to pass.
- [x] Run focused tests and verify GREEN:

```bash
cargo test -p continuitydb-steward default_steward_evaluation_suite --features local-model
cargo test -p continuitydb-cli cli_local_model_evaluation_suite_outputs_case_contracts --features local-model
cargo test -p continuitydb-cli cli_benchmark_local_model --features local-model
```

### Task 3: Documentation and Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-default-policy-rejection-evaluation-case.md`

- [x] Add README current-scope bullet for the default policy-rejection evaluation case.
- [x] Add roadmap Steward milestone for the default policy-rejection evaluation case.
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
git commit -m "feat: add policy rejection steward evaluation"
```
