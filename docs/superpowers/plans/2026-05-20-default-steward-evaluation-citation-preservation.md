# Default Steward Evaluation Citation Preservation Implementation Plan

**Goal:** Expand the fixed local Steward evaluation suite with a multi-source citation preservation case.

**Architecture:** Add one deterministic `StewardEvaluationCase` to `default_steward_evaluation_suite()`. Existing fingerprinting, CLI export, prompt artifact generation, benchmark reporting, and baseline recording already derive from the default suite.

**Tech Stack:** Rust, existing `continuitydb-steward` local-model feature, existing CLI JSON tests.

### Task 1: Red Tests

**Files:**
- Modify: `crates/continuitydb-steward/src/lib.rs`
- Modify: `crates/continuitydb-cli/tests/cli.rs`

- [x] Add expectations that the default suite has four cases and exposes `multi-source citation preservation`.
- [x] Add a valid default-suite response with the fourth `MarkFrontier` proposal and two citations.
- [x] Add CLI suite-export assertions for both required citations.
- [x] Add prompt artifact assertions for both evidence locators.
- [x] Verify red with focused steward and CLI tests.

### Task 2: Implementation

**Files:**
- Modify: `crates/continuitydb-steward/src/local_model.rs`

- [x] Add the multi-source citation preservation case to `default_steward_evaluation_suite()`.
- [x] Keep schema, grammar, policy, and benchmark storage unchanged.
- [x] Verify focused steward and CLI tests pass.

### Task 3: Documentation and Gate

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Add: `docs/superpowers/specs/2026-05-20-default-steward-evaluation-citation-preservation-design.md`
- Add: `docs/superpowers/plans/2026-05-20-default-steward-evaluation-citation-preservation.md`

- [x] Update README and roadmap.
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
git commit -m "feat: add citation preservation steward evaluation"
```
