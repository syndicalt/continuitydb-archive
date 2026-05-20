# Small Model Candidate Benchmark Arguments Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add deterministic candidate-recommended benchmark runner arguments for local Steward model evaluation.

**Architecture:** Add a `SmallModelCandidate` helper that creates a `LocalExecutableRunnerConfig` using the existing `LlamaCppRuntimeProfile` path. Expose its deterministic argument vector in CLI candidate JSON with placeholder paths.

**Tech Stack:** Rust, existing `continuitydb-steward` local-model feature tests, existing `continuitydb-cli` JSON tests, serde_json.

---

### Task 1: Add Failing Steward Runner Config Test

**Files:**
- Modify: `crates/continuitydb-steward/src/lib.rs`

- [x] **Step 1: Add recommended runner config test**

Add a feature-gated test near `small_model_candidates_expose_runtime_metadata`:

```rust
#[cfg(feature = "local-model")]
#[test]
fn small_model_candidate_builds_recommended_runner_config() {
    let config = small_model_candidates()[0]
        .recommended_runner_config("llama-cli", "/models/qwen.gguf");

    assert_eq!(config.executable(), std::path::Path::new("llama-cli"));
    assert_eq!(
        config.command_arguments(),
        vec![
            "--model".to_string(),
            "/models/qwen.gguf".to_string(),
            "--ctx-size".to_string(),
            "4096".to_string(),
            "--temp".to_string(),
            "0".to_string(),
            "--prompt".to_string(),
            "-".to_string(),
        ]
    );
}
```

- [x] **Step 2: Run steward focused test to verify RED**

Run:

```bash
cargo test -p continuitydb-steward small_model_candidate_builds_recommended_runner_config --features local-model
```

Expected: compile failure because `recommended_runner_config` does not exist.

### Task 2: Implement Candidate Runner Config Helper

**Files:**
- Modify: `crates/continuitydb-steward/src/local_model.rs`

- [x] **Step 1: Add helper method to `SmallModelCandidate`**

Add:

```rust
pub fn recommended_runner_config(
    &self,
    executable: impl Into<PathBuf>,
    model_path: impl Into<PathBuf>,
) -> LocalExecutableRunnerConfig {
    LlamaCppRuntimeProfile::new(executable, model_path)
        .with_temperature(format_temperature(self.recommended_temperature_millis))
        .runner_config()
}
```

- [x] **Step 2: Add deterministic temperature formatter**

Add near the fingerprint helpers:

```rust
fn format_temperature(temperature_millis: u16) -> String {
    if temperature_millis == 0 {
        return "0".to_string();
    }
    let whole = temperature_millis / 1000;
    let fractional = temperature_millis % 1000;
    format!("{whole}.{fractional:03}")
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string()
}
```

- [x] **Step 3: Run steward focused test to verify GREEN**

Run:

```bash
cargo test -p continuitydb-steward small_model_candidate_builds_recommended_runner_config --features local-model
```

Expected: PASS.

### Task 3: Expose Recommended Arguments In CLI Candidate JSON

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`
- Modify: `crates/continuitydb-cli/tests/cli.rs`
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-small-model-candidate-benchmark-arguments.md`

- [x] **Step 1: Add CLI JSON assertion**

In `cli_local_model_candidates_outputs_fixed_registry`, assert:

```rust
assert_eq!(
    json["candidates"][0]["recommended_runner_arguments"][0].as_str(),
    Some("--model")
);
assert_eq!(
    json["candidates"][0]["recommended_runner_arguments"][1].as_str(),
    Some("<model.gguf>")
);
assert_eq!(
    json["candidates"][0]["recommended_runner_arguments"][2].as_str(),
    Some("--ctx-size")
);
assert_eq!(
    json["candidates"][0]["recommended_runner_arguments"][5].as_str(),
    Some("0")
);
```

- [x] **Step 2: Run CLI focused test to verify RED**

Run:

```bash
cargo test -p continuitydb-cli cli_local_model_candidates_outputs_fixed_registry --features local-model
```

Expected: failure because `recommended_runner_arguments` is missing.

- [x] **Step 3: Add CLI JSON field**

In `local_model_candidates_json()`, build:

```rust
let recommended_config =
    candidate.recommended_runner_config("llama-cli", "<model.gguf>");
```

and include:

```rust
"recommended_runner_arguments": recommended_config.command_arguments(),
```

- [x] **Step 4: Run CLI focused test to verify GREEN**

Run:

```bash
cargo test -p continuitydb-cli cli_local_model_candidates_outputs_fixed_registry --features local-model
```

Expected: PASS.

- [x] **Step 5: Update README current scope**

Add:

```markdown
- Small local Steward model benchmark argument templates.
```

- [x] **Step 6: Update Steward roadmap**

Add after milestone 42:

```markdown
43. Add small-model benchmark argument templates. Added candidate-recommended runner configuration helpers and exposed deterministic benchmark argument vectors through CLI candidate JSON.
```

- [x] **Step 7: Mark this plan complete**

Check off completed steps in this plan before commit.

- [x] **Step 8: Run the full verification gate**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

Expected: all commands exit 0.

- [x] **Step 9: Commit**

Run:

```bash
git add README.md docs/roadmap.md docs/superpowers/specs/2026-05-20-small-model-candidate-benchmark-arguments-design.md docs/superpowers/plans/2026-05-20-small-model-candidate-benchmark-arguments.md crates/continuitydb-steward/src/local_model.rs crates/continuitydb-steward/src/lib.rs crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs
git commit -m "feat: expose small model benchmark arguments"
```
