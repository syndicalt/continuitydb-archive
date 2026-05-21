# Local Model Baseline Preflight Compatibility Evidence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Expose the matched durable baseline's compatibility identity in local-model dry-run baseline preflight JSON.

**Architecture:** `local_model_baseline_preflight_json` already resolves the latest compatible `LocalModelBenchmarkBaseline`. Project the matched baseline's existing accessors into `baseline_preflight` so dry-runs can report the exact schema, suite, prompt, and runtime identity that matched, while leaving compatibility filtering unchanged.

**Tech Stack:** Rust workspace, `continuitydb-cli`, serde JSON, `assert_cmd`, cargo tests with the `local-model` feature.

---

### Task 1: Add baseline preflight compatibility evidence

**Files:**
- Modify: `crates/continuitydb-cli/tests/cli.rs`
- Modify: `crates/continuitydb-cli/src/main.rs`
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [x] **Step 1: Write failing compatible-baseline assertions**

Extend `cli_benchmark_local_model_dry_run_reports_compatible_baseline_preflight` after the existing previous byte assertions with:

```rust
assert_eq!(
    json["baseline_preflight"]["previous_response_schema_version"].as_u64(),
    json["response_schema_version"].as_u64()
);
assert_eq!(
    json["baseline_preflight"]["previous_evaluation_suite_fingerprint"].as_str(),
    json["evaluation_suite_fingerprint"].as_str()
);
assert_eq!(
    json["baseline_preflight"]["previous_schema_fingerprint"].as_str(),
    json["schema_fingerprint"].as_str()
);
assert_eq!(
    json["baseline_preflight"]["previous_grammar_fingerprint"].as_str(),
    json["grammar_fingerprint"].as_str()
);
assert_eq!(
    json["baseline_preflight"]["previous_prompt_fingerprint"].as_str(),
    json["prompt_fingerprint"].as_str()
);
assert_eq!(
    json["baseline_preflight"]["previous_runtime"]["executable"].as_str(),
    json["runtime"]["executable"].as_str()
);
assert_eq!(
    json["baseline_preflight"]["previous_runtime"]["arguments"],
    json["runtime"]["arguments"]
);
```

- [x] **Step 2: Write failing missing-baseline null assertions**

Extend `cli_benchmark_local_model_dry_run_compare_reports_missing_baseline_without_creating_file` after the previous byte null assertions with:

```rust
assert!(json["baseline_preflight"]["previous_response_schema_version"].is_null());
assert!(json["baseline_preflight"]["previous_evaluation_suite_fingerprint"].is_null());
assert!(json["baseline_preflight"]["previous_schema_fingerprint"].is_null());
assert!(json["baseline_preflight"]["previous_grammar_fingerprint"].is_null());
assert!(json["baseline_preflight"]["previous_prompt_fingerprint"].is_null());
assert!(json["baseline_preflight"]["previous_runtime"].is_null());
```

- [x] **Step 3: Run focused test to verify RED**

Run:

```sh
cargo test -p continuitydb-cli --features local-model cli_benchmark_local_model_dry_run_reports_compatible_baseline_preflight --test cli
```

Expected: fail because `baseline_preflight` does not include previous compatibility identity fields yet.

- [x] **Step 4: Implement preflight compatibility output**

In `local_model_baseline_preflight_json`, add a local runtime projection:

```rust
let previous_runtime = latest.as_ref().map(|baseline| {
    serde_json::json!({
        "executable": baseline.runtime().executable(),
        "arguments": baseline.runtime().arguments(),
    })
});
```

Then add these fields to the returned JSON:

```rust
"previous_response_schema_version": latest
    .as_ref()
    .map(LocalModelBenchmarkBaseline::response_schema_version),
"previous_evaluation_suite_fingerprint": latest
    .as_ref()
    .map(LocalModelBenchmarkBaseline::evaluation_suite_fingerprint),
"previous_schema_fingerprint": latest
    .as_ref()
    .map(LocalModelBenchmarkBaseline::schema_fingerprint),
"previous_grammar_fingerprint": latest
    .as_ref()
    .map(LocalModelBenchmarkBaseline::grammar_fingerprint),
"previous_prompt_fingerprint": latest
    .as_ref()
    .map(LocalModelBenchmarkBaseline::prompt_fingerprint),
"previous_runtime": previous_runtime,
```

- [x] **Step 5: Run focused tests to verify GREEN**

Run:

```sh
cargo test -p continuitydb-cli --features local-model cli_benchmark_local_model_dry_run_reports_compatible_baseline_preflight --test cli
cargo test -p continuitydb-cli --features local-model cli_benchmark_local_model_dry_run_compare_reports_missing_baseline_without_creating_file --test cli
```

Expected: pass.

- [x] **Step 6: Update docs**

Add `CLI local model dry-run baseline compatibility evidence` to `README.md` current scope after dry-run baseline byte evidence. Add roadmap item 115 after dry-run baseline byte evidence.

- [x] **Step 7: Run verification gates**

Run:

```sh
cargo fmt --all
cargo fmt --all -- --check
git diff --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
```

- [ ] **Step 8: Commit**

Commit:

```sh
git add README.md crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs docs/roadmap.md docs/superpowers/plans/2026-05-20-local-model-baseline-preflight-compatibility-evidence.md docs/superpowers/specs/2026-05-20-local-model-baseline-preflight-compatibility-evidence-design.md
git commit -m "feat: report local-model preflight compatibility evidence"
```
