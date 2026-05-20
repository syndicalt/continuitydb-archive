# Local Model Prompt Fingerprint Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Persist deterministic local Steward prompt fingerprints on benchmark reports and baselines, and use them in compatibility filtering.

**Architecture:** Compute one suite-level prompt fingerprint from ordered rendered prompts in `continuitydb-steward`. Store it beside existing schema, grammar, and suite fingerprints. Surface it in CLI JSON using the baseline/dry-run metadata path.

**Tech Stack:** Rust, serde, assert_cmd CLI tests, deterministic FNV-1a fingerprint helpers.

---

### Task 1: Steward Tests

**Files:**
- Modify: `crates/continuitydb-steward/src/lib.rs`

- [x] **Step 1: Add failing report/baseline persistence test**

Add assertions to the local-model test module:

```rust
#[cfg(feature = "local-model")]
#[test]
fn local_model_benchmark_report_and_baseline_preserve_prompt_fingerprint(
) -> Result<(), Box<dyn std::error::Error>> {
    let benchmark = LocalModelBenchmark::new(
        small_model_candidates()[0],
        LocalExecutableRunner::new(LocalExecutableRunnerConfig::new("llama-cli")),
        default_steward_evaluation_suite(),
    );

    let report = benchmark.run(steward()?);
    let baseline = LocalModelBenchmarkBaseline::from_report(report.clone(), created_at());

    assert!(report.prompt_fingerprint().starts_with("fnv1a64:"));
    assert_eq!(baseline.prompt_fingerprint(), report.prompt_fingerprint());
    Ok(())
}
```

- [x] **Step 2: Add failing legacy decode test**

Add:

```rust
#[cfg(feature = "local-model")]
#[test]
fn local_model_benchmark_baseline_decodes_legacy_json_without_prompt_fingerprint(
) -> Result<(), Box<dyn std::error::Error>> {
    let encoded = serde_json::json!({
        "candidate_model_id": "Qwen/Qwen2.5-0.5B-Instruct",
        "candidate_role": "default-feasibility",
        "response_schema_version": LOCAL_MODEL_RESPONSE_SCHEMA_VERSION,
        "evaluation_suite_fingerprint": default_steward_evaluation_suite().fingerprint(),
        "schema_fingerprint": "fnv1a64:1111111111111111",
        "grammar_fingerprint": "fnv1a64:2222222222222222",
        "runtime": { "executable": "sh", "arguments": [] },
        "evaluation": { "case_reports": [] },
        "recorded_at": created_at(),
    });

    let baseline: LocalModelBenchmarkBaseline = serde_json::from_value(encoded)?;

    assert_eq!(baseline.prompt_fingerprint(), "");
    Ok(())
}
```

- [x] **Step 3: Add failing compatibility test**

Add:

```rust
#[cfg(feature = "local-model")]
#[test]
fn latest_compatible_local_model_baseline_requires_prompt_fingerprint(
) -> Result<(), Box<dyn std::error::Error>> {
    let old_compatible = local_model_empty_baseline(
        small_model_candidates()[0],
        Utc.with_ymd_and_hms(2026, 5, 20, 1, 0, 0)
            .single()
            .unwrap_or_else(Utc::now),
    )?;
    let mut incompatible_json = serde_json::to_value(local_model_empty_baseline(
        small_model_candidates()[0],
        Utc.with_ymd_and_hms(2026, 5, 20, 2, 0, 0)
            .single()
            .unwrap_or_else(Utc::now),
    )?)?;
    incompatible_json["prompt_fingerprint"] = serde_json::json!("fnv1a64:0000000000000000");
    let incompatible_prompt: LocalModelBenchmarkBaseline =
        serde_json::from_value(incompatible_json)?;
    let current = local_model_empty_baseline(
        small_model_candidates()[0],
        Utc.with_ymd_and_hms(2026, 5, 20, 3, 0, 0)
            .single()
            .unwrap_or_else(Utc::now),
    )?;
    let mut store = MemoryLocalModelBenchmarkBaselineStore::default();
    store.append_baseline(old_compatible.clone())?;
    store.append_baseline(incompatible_prompt)?;

    let latest = latest_compatible_local_model_benchmark_baseline(&store, &current)?;

    assert_eq!(latest, Some(old_compatible));
    Ok(())
}
```

- [x] **Step 4: Verify RED**

Run:

```bash
cargo test -p continuitydb-steward prompt_fingerprint --features local-model
```

Expected: FAIL because prompt fingerprint APIs do not exist.

### Task 2: Steward Implementation

**Files:**
- Modify: `crates/continuitydb-steward/src/local_model.rs`

- [x] **Step 1: Add prompt fingerprint helper**

Compute the suite-level prompt fingerprint from ordered rendered prompts:

```rust
fn prompt_fingerprint_for_suite(suite: &StewardEvaluationSuite) -> String {
    let fields = suite
        .cases()
        .iter()
        .map(|case| local_model_prompt_for_input(case.input()))
        .collect::<Vec<_>>();
    fingerprint_fields(&fields)
}
```

- [x] **Step 2: Add report and baseline fields/accessors**

Add `prompt_fingerprint: String` to `LocalModelBenchmarkReport` and `LocalModelBenchmarkBaseline`, defaulting the baseline field for legacy decode.

- [x] **Step 3: Add compatibility filter**

Require `baseline.prompt_fingerprint() == current.prompt_fingerprint()` inside `latest_compatible_local_model_benchmark_baseline`.

- [x] **Step 4: Verify GREEN**

Run:

```bash
cargo test -p continuitydb-steward prompt_fingerprint --features local-model
```

Expected: PASS.

### Task 3: CLI Output

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`
- Modify: `crates/continuitydb-cli/tests/cli.rs`

- [x] **Step 1: Add failing CLI assertions**

Update local-model dry-run and benchmark recording tests to assert `prompt_fingerprint` starts with `fnv1a64:`.

- [x] **Step 2: Add CLI output fields**

Add `prompt_fingerprint` to dry-run and benchmark JSON output.

- [x] **Step 3: Verify CLI tests**

Run:

```bash
cargo test -p continuitydb-cli prompt_fingerprint --features local-model
```

Expected: PASS.

### Task 4: Documentation and Full Gate

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-local-model-prompt-fingerprint.md`

- [x] **Step 1: Update README**

Add:

```markdown
- Durable local model prompt fingerprints.
```

- [x] **Step 2: Update roadmap**

Add Steward milestone 49:

```markdown
49. Add durable local-model prompt fingerprints. Persisted deterministic prompt-rendering fingerprints on benchmark reports and baselines, exposed them in CLI benchmark JSON, and required matching fingerprints for compatible baseline regression gates.
```

- [x] **Step 3: Run full verification**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

Expected: all commands exit 0.

- [x] **Step 4: Commit**

Run:

```bash
git add README.md docs/roadmap.md docs/superpowers/specs/2026-05-20-local-model-prompt-fingerprint-design.md docs/superpowers/plans/2026-05-20-local-model-prompt-fingerprint.md crates/continuitydb-steward/src/local_model.rs crates/continuitydb-steward/src/lib.rs crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs
git commit -m "feat: persist local model prompt fingerprints"
```
