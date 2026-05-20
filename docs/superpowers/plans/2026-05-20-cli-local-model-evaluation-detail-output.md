# CLI Local Model Evaluation Detail Output Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add serialized per-case local Steward evaluation details to `benchmark-local-model` JSON output.

**Architecture:** Reuse the existing serializable `StewardEvaluationReport` from the current baseline and embed it under an `evaluation` key in the CLI JSON object. Keep top-level summary fields intact.

**Tech Stack:** Rust 2021, `continuitydb-cli`, feature-gated `local-model`, `serde_json`.

---

### Task 1: CLI Evaluation Detail Output

**Files:**
- Modify: `crates/continuitydb-cli/tests/cli.rs`
- Modify: `crates/continuitydb-cli/src/main.rs`
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [x] **Step 1: Write failing CLI assertions**

In `cli_benchmark_local_model_records_baseline`, add assertions:

```rust
assert_eq!(
    json["evaluation"]["case_reports"][0]["name"].as_str(),
    Some("insufficient evidence uncertainty")
);
assert_eq!(
    json["evaluation"]["case_reports"][0]["failures"]
        .as_array()
        .map(Vec::len),
    Some(0)
);
```

- [x] **Step 2: Verify RED**

Run:

```bash
cargo test -p continuitydb-cli cli_benchmark_local_model_records_baseline --features local-model
```

Expected: test failure because `evaluation.case_reports` is absent from CLI output.

- [x] **Step 3: Implement CLI output**

In `local_model_benchmark_json`, add:

```rust
"evaluation": baseline.evaluation(),
```

to the top-level JSON object.

- [x] **Step 4: Verify GREEN**

Run:

```bash
cargo test -p continuitydb-cli cli_benchmark_local_model_records_baseline --features local-model
```

Expected: test passes.

- [x] **Step 5: Update docs**

Add README current-scope bullet:

```markdown
- CLI local model per-case evaluation detail output.
```

Add Steward milestone:

```markdown
32. Add CLI local-model per-case evaluation detail output. Embedded serialized Steward evaluation reports in `benchmark-local-model` JSON so operator artifacts carry case names and deterministic failure reasons alongside aggregate metrics.
```

- [x] **Step 6: Run full gate**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

Expected: all commands pass.

- [x] **Step 7: Commit**

Run:

```bash
git add README.md docs/roadmap.md docs/superpowers/specs/2026-05-20-cli-local-model-evaluation-detail-output-design.md docs/superpowers/plans/2026-05-20-cli-local-model-evaluation-detail-output.md crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs
git commit -m "feat: expose local model evaluation details in cli"
```
