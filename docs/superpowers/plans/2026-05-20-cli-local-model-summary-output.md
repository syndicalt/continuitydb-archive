# CLI Local Model Summary Output Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Route `benchmark-local-model` JSON summary fields through the public local model evaluation summary API.

**Architecture:** Keep the existing CLI command and output shape, add `failed_cases` and `pass_rate`, and replace manual case counting with `LocalModelBenchmarkBaseline::evaluation_summary()`.

**Tech Stack:** Rust 2021, `continuitydb-cli`, feature-gated `local-model`, `serde_json`.

---

### Task 1: CLI Summary Output

**Files:**
- Modify: `crates/continuitydb-cli/tests/cli.rs`
- Modify: `crates/continuitydb-cli/src/main.rs`
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [x] **Step 1: Write failing CLI assertions**

In `cli_benchmark_local_model_records_baseline`, add assertions:

```rust
assert_eq!(json["failed_cases"].as_u64(), Some(0));
assert_eq!(json["pass_rate"].as_f64(), Some(1.0));
```

- [x] **Step 2: Verify RED**

Run:

```bash
cargo test -p continuitydb-cli cli_benchmark_local_model_records_baseline --features local-model
```

Expected: test failure because `failed_cases` and `pass_rate` are absent from CLI output.

- [x] **Step 3: Implement CLI output via summary API**

In `local_model_benchmark_json`, replace manual case counting with:

```rust
let summary = baseline.evaluation_summary();
```

Emit:

```rust
"passed": summary.passed(),
"passed_cases": summary.passed_cases(),
"failed_cases": summary.failed_cases(),
"total_cases": summary.total_cases(),
"pass_rate": summary.pass_rate(),
```

- [x] **Step 4: Verify GREEN**

Run:

```bash
cargo test -p continuitydb-cli cli_benchmark_local_model_records_baseline --features local-model
```

Expected: test passes.

- [x] **Step 5: Update docs**

Add README current-scope bullet:

```markdown
- CLI local model evaluation summary output.
```

Add Steward milestone:

```markdown
31. Add CLI local-model evaluation summary output. Routed `benchmark-local-model` JSON through the public evaluation summary API and exposed failed-case and pass-rate metrics alongside existing pass counts.
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
git add README.md docs/roadmap.md docs/superpowers/specs/2026-05-20-cli-local-model-summary-output-design.md docs/superpowers/plans/2026-05-20-cli-local-model-summary-output.md crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs
git commit -m "feat: expose local model summary in cli"
```
