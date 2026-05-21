# Local Model Baseline Preflight Byte Evidence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add matched-baseline schema and grammar byte evidence to local-model dry-run baseline preflight JSON.

**Architecture:** Reuse the `LocalModelBenchmarkBaseline::schema_bytes` and `grammar_bytes` accessors added for durable baseline records. `local_model_baseline_preflight_json` already resolves the latest compatible baseline, so it can project those byte counts into the preflight object without changing compatibility filtering or baseline file mutation behavior.

**Tech Stack:** Rust workspace, `continuitydb-cli`, serde JSON, `assert_cmd`, cargo tests with the `local-model` feature.

---

### Task 1: Add baseline preflight byte evidence

**Files:**
- Modify: `crates/continuitydb-cli/tests/cli.rs`
- Modify: `crates/continuitydb-cli/src/main.rs`
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [x] **Step 1: Write failing compatible-baseline preflight assertions**

Extend `cli_benchmark_local_model_dry_run_reports_compatible_baseline_preflight` after the `previous_recorded_at` assertion with:

```rust
assert_eq!(
    json["baseline_preflight"]["previous_schema_bytes"].as_u64(),
    json["schema_bytes"].as_u64()
);
assert_eq!(
    json["baseline_preflight"]["previous_grammar_bytes"].as_u64(),
    json["grammar_bytes"].as_u64()
);
```

- [x] **Step 2: Write failing missing-baseline null assertions**

Extend `cli_benchmark_local_model_dry_run_compare_reports_missing_baseline_without_creating_file` after the `previous_recorded_at` assertion with:

```rust
assert!(json["baseline_preflight"]["previous_schema_bytes"].is_null());
assert!(json["baseline_preflight"]["previous_grammar_bytes"].is_null());
```

- [x] **Step 3: Run focused tests to verify RED**

Run:

```sh
cargo test -p continuitydb-cli --features local-model cli_benchmark_local_model_dry_run_reports_compatible_baseline_preflight --test cli
```

Expected: fail because `baseline_preflight` does not include previous contract byte fields yet.

- [x] **Step 4: Implement preflight byte output**

In `local_model_baseline_preflight_json`, add:

```rust
"previous_schema_bytes": latest.as_ref().map(LocalModelBenchmarkBaseline::schema_bytes),
"previous_grammar_bytes": latest.as_ref().map(LocalModelBenchmarkBaseline::grammar_bytes),
```

next to `previous_recorded_at`.

- [x] **Step 5: Run focused tests to verify GREEN**

Run:

```sh
cargo test -p continuitydb-cli --features local-model cli_benchmark_local_model_dry_run_reports_compatible_baseline_preflight --test cli
cargo test -p continuitydb-cli --features local-model cli_benchmark_local_model_dry_run_compare_reports_missing_baseline_without_creating_file --test cli
```

Expected: pass.

- [x] **Step 6: Update docs**

Add `CLI local model dry-run baseline byte evidence` to `README.md` current scope after dry-run baseline compatibility preflight. Add a corresponding Steward roadmap item after durable local-model contract byte metadata.

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
git add README.md crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs docs/roadmap.md docs/superpowers/plans/2026-05-20-local-model-baseline-preflight-byte-evidence.md docs/superpowers/specs/2026-05-20-local-model-baseline-preflight-byte-evidence-design.md
git commit -m "feat: report local-model preflight baseline bytes"
```
