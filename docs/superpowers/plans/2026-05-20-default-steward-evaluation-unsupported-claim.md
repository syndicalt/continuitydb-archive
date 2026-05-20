# Default Steward Evaluation Unsupported Claim Implementation Plan

**Goal:** Expand the fixed local Steward evaluation suite with an unsupported-claim boundary case.

**Architecture:** Add one deterministic `StewardEvaluationCase` to `default_steward_evaluation_suite()`. Existing suite fingerprinting, CLI export, prompt artifact generation, benchmark reports, and baselines already derive from the default suite, so the implementation should reuse those paths instead of adding special cases.

**Tech Stack:** Rust, existing `continuitydb-steward` local-model feature, existing CLI JSON tests.

### Task 1: Red Tests

**Files:**
- Modify: `crates/continuitydb-steward/src/lib.rs`
- Modify: `crates/continuitydb-cli/tests/cli.rs`

- [x] Add expectations that the default suite has three cases and exposes `unsupported claim boundary`.
- [x] Add a valid default-suite response with the third `RequestVerification` proposal.
- [x] Add CLI suite-export assertions for the third case.
- [x] Verify red with focused steward and CLI tests.

### Task 2: Implementation

**Files:**
- Modify: `crates/continuitydb-steward/src/local_model.rs`

- [x] Add the unsupported-claim `StewardEvaluationCase` to `default_steward_evaluation_suite()`.
- [x] Keep schema, grammar, policy, and benchmark storage unchanged.
- [x] Verify focused steward and CLI tests pass.

### Task 3: Derived Test Updates

**Files:**
- Modify: `crates/continuitydb-cli/tests/cli.rs`

- [x] Update benchmark fixture output to emit all three expected proposals.
- [x] Update prompt artifact assertions to expect three prompt files.
- [x] Verify focused CLI benchmark and prompt tests pass.

### Task 4: Documentation and Gate

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Add: `docs/superpowers/specs/2026-05-20-default-steward-evaluation-unsupported-claim-design.md`
- Add: `docs/superpowers/plans/2026-05-20-default-steward-evaluation-unsupported-claim.md`

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
git commit -m "feat: add unsupported claim steward evaluation"
```
