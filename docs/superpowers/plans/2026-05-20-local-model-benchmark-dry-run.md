# Local Model Benchmark Dry Run Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `benchmark-local-model --dry-run` so operators can inspect the exact local-model benchmark candidate/runtime/suite artifact without executing a model or writing baselines.

**Architecture:** Extend the existing `BenchmarkLocalModel` command with a `dry_run` flag. Reuse existing candidate validation, runner argument construction, response schema version, and default evaluation suite fingerprint; branch before opening the baseline store or constructing the executable runner.

**Tech Stack:** Rust, clap, `serde_json`, existing feature-gated `continuitydb-cli` local-model tests.

---

### Task 1: Add Benchmark Dry-Run Behavior

**Files:**
- Modify: `crates/continuitydb-cli/tests/cli.rs`
- Modify: `crates/continuitydb-cli/src/main.rs`

- [x] **Step 1: Write the failing dry-run CLI test**

Add this test near `cli_benchmark_local_model_records_baseline`:

```rust
#[cfg(feature = "local-model")]
#[test]
fn cli_benchmark_local_model_dry_run_outputs_preflight_without_baseline(
) -> Result<(), Box<dyn std::error::Error>> {
    let baseline_path = temp_store_path("continuitydb-cli-local-model-dry-run-baseline");

    let output = Command::cargo_bin("continuitydb")?
        .arg("benchmark-local-model")
        .arg("--dry-run")
        .arg("--candidate")
        .arg("Qwen/Qwen2.5-0.5B-Instruct")
        .arg("--executable")
        .arg("/missing/local-model-runner")
        .arg("--model-path")
        .arg("/models/qwen.gguf")
        .arg("--arg")
        .arg("--temp")
        .arg("--arg")
        .arg("0")
        .arg("--baseline-path")
        .arg(&baseline_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(json["dry_run"].as_bool(), Some(true));
    assert_eq!(json["will_record_baseline"].as_bool(), Some(false));
    assert_eq!(
        json["candidate_model_id"].as_str(),
        Some("Qwen/Qwen2.5-0.5B-Instruct")
    );
    assert_eq!(json["candidate_role"].as_str(), Some("default-feasibility"));
    assert_eq!(json["response_schema_version"].as_u64(), Some(1));
    assert!(json["evaluation_suite_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
    assert_eq!(
        json["baseline_path"].as_str(),
        Some(baseline_path.display().to_string().as_str())
    );
    assert_eq!(
        json["runtime"]["executable"].as_str(),
        Some("/missing/local-model-runner")
    );
    assert_eq!(json["runtime"]["arguments"][0].as_str(), Some("--model"));
    assert_eq!(
        json["runtime"]["arguments"][1].as_str(),
        Some("/models/qwen.gguf")
    );
    assert_eq!(json["runtime"]["arguments"][2].as_str(), Some("--temp"));
    assert_eq!(json["runtime"]["arguments"][3].as_str(), Some("0"));
    assert!(!baseline_path.exists());
    Ok(())
}
```

- [x] **Step 2: Run the focused test to verify RED**

Run:

```bash
cargo test -p continuitydb-cli cli_benchmark_local_model_dry_run_outputs_preflight_without_baseline --features local-model
```

Expected: FAIL because `--dry-run` is not accepted by `benchmark-local-model`.

- [x] **Step 3: Add the minimal CLI implementation**

Add a `dry_run: bool` field to `BenchmarkLocalModel`:

```rust
/// Print benchmark configuration without executing the model or recording a baseline.
#[arg(long = "dry-run")]
dry_run: bool,
```

Pass it into `benchmark_local_model_json`.

Update `benchmark_local_model_json` to accept `dry_run: bool`, build the candidate and config first, and return this helper when `dry_run` is true:

```rust
#[cfg(feature = "local-model")]
fn local_model_benchmark_dry_run_json(
    candidate: SmallModelCandidate,
    config: &LocalExecutableRunnerConfig,
    baseline_path: &Path,
) -> serde_json::Value {
    serde_json::json!({
        "dry_run": true,
        "will_record_baseline": false,
        "candidate_model_id": candidate.model_id(),
        "candidate_role": candidate.role(),
        "baseline_path": baseline_path.display().to_string(),
        "response_schema_version": LOCAL_MODEL_RESPONSE_SCHEMA_VERSION,
        "evaluation_suite_fingerprint": default_steward_evaluation_suite().fingerprint(),
        "runtime": {
            "executable": config.executable().display().to_string(),
            "arguments": config.command_arguments(),
        },
    })
}
```

Only construct `LocalExecutableRunner`, open `FileLocalModelBenchmarkBaselineStore`, and record a baseline when `dry_run` is false.

- [x] **Step 4: Run focused tests to verify GREEN**

Run:

```bash
cargo test -p continuitydb-cli cli_benchmark_local_model_dry_run_outputs_preflight_without_baseline --features local-model
cargo test -p continuitydb-cli cli_benchmark_local_model_records_baseline --features local-model
```

Expected: both PASS.

### Task 2: Update Documentation And Verify

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-local-model-benchmark-dry-run.md`

- [x] **Step 1: Update README current scope**

Add:

```markdown
- CLI local model benchmark dry-run preflight output.
```

- [x] **Step 2: Update Steward roadmap**

Add this milestone after CLI small-model candidate registry output:

```markdown
39. Add CLI local-model benchmark dry-run preflight output. Added `benchmark-local-model --dry-run` so operators can inspect candidate, runtime, schema, suite fingerprint, and baseline target metadata without executing a model or mutating baseline records.
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
git add README.md docs/roadmap.md docs/superpowers/specs/2026-05-20-local-model-benchmark-dry-run-design.md docs/superpowers/plans/2026-05-20-local-model-benchmark-dry-run.md crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs
git commit -m "feat: add local model benchmark dry run"
```
