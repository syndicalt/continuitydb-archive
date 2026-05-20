# CLI Local Model Evaluation Suite Fingerprint Output Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the deterministic evaluation-suite fingerprint to the feature-gated `local-model-evaluation-suite` CLI JSON output.

**Architecture:** Reuse the existing `StewardEvaluationSuite::fingerprint()` method inside the CLI JSON construction helper. Keep the fingerprint top-level and named `evaluation_suite_fingerprint` to match benchmark output and baseline records.

**Tech Stack:** Rust, `serde_json`, existing `continuitydb-cli` local-model feature tests.

---

### Task 1: Add Fingerprint To Suite Inspection JSON

**Files:**
- Modify: `crates/continuitydb-cli/tests/cli.rs`
- Modify: `crates/continuitydb-cli/src/main.rs`

- [x] **Step 1: Write the failing CLI test assertion**

Add this assertion to `cli_local_model_evaluation_suite_outputs_case_contracts` after the `total_cases` assertion:

```rust
assert!(
    json["evaluation_suite_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:"))
);
```

- [x] **Step 2: Run the focused test to verify RED**

Run:

```bash
cargo test -p continuitydb-cli cli_local_model_evaluation_suite_outputs_case_contracts --features local-model
```

Expected: FAIL because `evaluation_suite_fingerprint` is missing from `local-model-evaluation-suite` output.

- [x] **Step 3: Add the minimal CLI implementation**

Update `local_model_evaluation_suite_json()` in `crates/continuitydb-cli/src/main.rs`:

```rust
serde_json::json!({
    "response_schema_version": LOCAL_MODEL_RESPONSE_SCHEMA_VERSION,
    "evaluation_suite_fingerprint": suite.fingerprint(),
    "total_cases": suite.len(),
    "cases": cases,
})
```

- [x] **Step 4: Run the focused test to verify GREEN**

Run:

```bash
cargo test -p continuitydb-cli cli_local_model_evaluation_suite_outputs_case_contracts --features local-model
```

Expected: PASS.

### Task 2: Update Documentation And Verify

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-cli-local-model-evaluation-suite-fingerprint-output.md`

- [x] **Step 1: Update README current scope**

Add:

```markdown
- CLI local model evaluation suite fingerprint output.
```

- [x] **Step 2: Update the Steward roadmap**

Add this milestone after local-model evaluation suite fingerprints:

```markdown
37. Add CLI local-model evaluation suite fingerprint output. Included the deterministic suite fingerprint in the feature-gated `local-model-evaluation-suite` JSON so operators can match inspected benchmark contracts to recorded baselines.
```

- [x] **Step 3: Mark this plan complete**

Check off completed steps in this plan before commit.

- [x] **Step 4: Run the full verification gate**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

Expected: all commands exit 0.

- [ ] **Step 5: Commit**

Run:

```bash
git add README.md docs/roadmap.md docs/superpowers/specs/2026-05-20-cli-local-model-evaluation-suite-fingerprint-output-design.md docs/superpowers/plans/2026-05-20-cli-local-model-evaluation-suite-fingerprint-output.md crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs
git commit -m "feat: expose evaluation suite fingerprint in cli"
```
