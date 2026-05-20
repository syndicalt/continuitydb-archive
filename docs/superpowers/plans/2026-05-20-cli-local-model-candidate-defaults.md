# CLI Local Model Candidate Defaults Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let `benchmark-local-model` materialize selected candidate default benchmark arguments directly.

**Architecture:** Add a `candidate_defaults` option to the CLI command and options struct. Build the runner config through `SmallModelCandidate::recommended_runner_config` when enabled, then append explicit `--arg` values.

**Tech Stack:** Rust, clap, existing `continuitydb-cli` local-model dry-run tests, existing `continuitydb-steward` candidate runner helper.

---

### Task 1: Add Failing CLI Dry-Run Test

**Files:**
- Modify: `crates/continuitydb-cli/tests/cli.rs`

- [x] **Step 1: Add candidate defaults dry-run test**

Add near `cli_benchmark_local_model_dry_run_outputs_preflight_without_baseline`:

```rust
#[cfg(feature = "local-model")]
#[test]
fn cli_benchmark_local_model_dry_run_uses_candidate_defaults(
) -> Result<(), Box<dyn std::error::Error>> {
    let baseline_path = temp_store_path("continuitydb-cli-local-model-defaults-baseline");

    let output = Command::cargo_bin("continuitydb")?
        .arg("benchmark-local-model")
        .arg("--dry-run")
        .arg("--candidate-defaults")
        .arg("--candidate")
        .arg("Qwen/Qwen2.5-0.5B-Instruct")
        .arg("--executable")
        .arg("/missing/local-model-runner")
        .arg("--model-path")
        .arg("/models/qwen.gguf")
        .arg("--arg")
        .arg("--threads")
        .arg("--arg")
        .arg("2")
        .arg("--baseline-path")
        .arg(&baseline_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(json["runtime"]["arguments"][0].as_str(), Some("--model"));
    assert_eq!(json["runtime"]["arguments"][1].as_str(), Some("/models/qwen.gguf"));
    assert_eq!(json["runtime"]["arguments"][2].as_str(), Some("--ctx-size"));
    assert_eq!(json["runtime"]["arguments"][3].as_str(), Some("4096"));
    assert_eq!(json["runtime"]["arguments"][4].as_str(), Some("--temp"));
    assert_eq!(json["runtime"]["arguments"][5].as_str(), Some("0"));
    assert_eq!(json["runtime"]["arguments"][6].as_str(), Some("--prompt"));
    assert_eq!(json["runtime"]["arguments"][7].as_str(), Some("-"));
    assert_eq!(json["runtime"]["arguments"][8].as_str(), Some("--threads"));
    assert_eq!(json["runtime"]["arguments"][9].as_str(), Some("2"));
    assert!(!baseline_path.exists());
    Ok(())
}
```

- [x] **Step 2: Run focused CLI test to verify RED**

Run:

```bash
cargo test -p continuitydb-cli cli_benchmark_local_model_dry_run_uses_candidate_defaults --features local-model
```

Expected: clap rejects unknown `--candidate-defaults`.

### Task 2: Implement CLI Candidate Defaults

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`

- [x] **Step 1: Add command field**

In `Command::BenchmarkLocalModel`, add:

```rust
/// Use the selected candidate's recommended benchmark arguments before extra --arg values.
#[arg(long = "candidate-defaults")]
candidate_defaults: bool,
```

- [x] **Step 2: Add options field**

In `LocalModelBenchmarkOptions`, add:

```rust
candidate_defaults: bool,
```

- [x] **Step 3: Pass option through command dispatch**

Pass `candidate_defaults` into `benchmark_local_model_json`.

- [x] **Step 4: Build config from candidate defaults**

Replace config construction in `benchmark_local_model_json` with:

```rust
let mut config = if options.candidate_defaults {
    candidate.recommended_runner_config(
        options.executable.to_path_buf(),
        options.model_path.to_path_buf(),
    )
} else {
    LocalExecutableRunnerConfig::new(options.executable.to_path_buf())
        .with_model_path(options.model_path.to_path_buf())
};
```

Keep the existing loop that appends explicit `--arg` values.

- [x] **Step 5: Run focused CLI test to verify GREEN**

Run:

```bash
cargo test -p continuitydb-cli cli_benchmark_local_model_dry_run_uses_candidate_defaults --features local-model
```

Expected: PASS.

### Task 3: Update Docs And Verify

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-cli-local-model-candidate-defaults.md`

- [x] **Step 1: Update README current scope**

Add:

```markdown
- CLI local model benchmark candidate defaults.
```

- [x] **Step 2: Update Steward roadmap**

Add after milestone 43:

```markdown
44. Add CLI local-model benchmark candidate defaults. Added `benchmark-local-model --candidate-defaults` so benchmark runs and dry-runs can materialize candidate-recommended runner arguments before operator-supplied extra arguments.
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

- [x] **Step 5: Commit**

Run:

```bash
git add README.md docs/roadmap.md docs/superpowers/specs/2026-05-20-cli-local-model-candidate-defaults-design.md docs/superpowers/plans/2026-05-20-cli-local-model-candidate-defaults.md crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs
git commit -m "feat: add local model candidate defaults"
```
