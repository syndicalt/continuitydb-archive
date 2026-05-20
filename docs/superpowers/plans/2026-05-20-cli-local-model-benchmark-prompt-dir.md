# CLI Local Model Benchmark Prompt Directory Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `benchmark-local-model --prompt-dir <DIR>` so operators can archive the exact local Steward evaluation prompts used by dry-runs and benchmark runs.

**Architecture:** Expose deterministic prompt rendering from `continuitydb-steward` and keep CLI artifact writing in `continuitydb-cli`. Prompt artifacts are generated from `default_steward_evaluation_suite()` before dry-run or run output is produced.

**Tech Stack:** Rust, clap, serde_json, assert_cmd CLI tests.

---

### Task 1: Prompt Directory CLI Test

**Files:**
- Modify: `crates/continuitydb-cli/tests/cli.rs`

- [x] **Step 1: Write the failing test**

Add a feature-gated test near the local-model dry-run tests:

```rust
#[cfg(feature = "local-model")]
#[test]
fn cli_benchmark_local_model_prompt_dir_writes_prompt_artifacts(
) -> Result<(), Box<dyn std::error::Error>> {
    let baseline_path = temp_store_path("continuitydb-cli-local-model-prompt-dir-baseline");
    let prompt_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-prompt-dir-{}",
        std::process::id()
    ));
    if prompt_dir.exists() {
        fs::remove_dir_all(&prompt_dir)?;
    }

    let output = Command::cargo_bin("continuitydb")?
        .arg("benchmark-local-model")
        .arg("--dry-run")
        .arg("--candidate")
        .arg("Qwen/Qwen2.5-0.5B-Instruct")
        .arg("--executable")
        .arg("/missing/local-model-runner")
        .arg("--model-path")
        .arg("/models/qwen.gguf")
        .arg("--prompt-dir")
        .arg(&prompt_dir)
        .arg("--baseline-path")
        .arg(&baseline_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;
    let artifacts = json["prompt_artifacts"]
        .as_array()
        .ok_or_else(|| std::io::Error::other("missing prompt artifacts"))?;

    assert_eq!(artifacts.len(), 2);
    assert_eq!(
        artifacts[0]["case_name"].as_str(),
        Some("insufficient evidence uncertainty")
    );
    assert!(artifacts[0]["prompt_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
    assert!(artifacts[0]["prompt_bytes"].as_u64().is_some_and(|bytes| bytes > 0));
    let first_prompt_path = artifacts[0]["prompt_path"]
        .as_str()
        .ok_or_else(|| std::io::Error::other("missing prompt path"))?;
    let first_prompt = fs::read_to_string(first_prompt_path)?;
    assert!(first_prompt.contains("You are the ContinuityDB database Steward."));
    assert!(first_prompt.contains("Assess whether thin evidence needs verification."));
    assert!(first_prompt.contains("continuitydb://evaluation/thin-evidence"));
    assert!(first_prompt.contains("One weak source mentions the claim without corroboration."));
    assert!(!baseline_path.exists());

    fs::remove_dir_all(prompt_dir)?;
    Ok(())
}
```

- [x] **Step 2: Verify RED**

Run:

```bash
cargo test -p continuitydb-cli cli_benchmark_local_model_prompt_dir_writes_prompt_artifacts --features local-model
```

Expected: FAIL because `--prompt-dir` is not accepted.

### Task 2: Steward Prompt Rendering Helper

**Files:**
- Modify: `crates/continuitydb-steward/src/local_model.rs`
- Modify: `crates/continuitydb-steward/src/lib.rs`

- [x] **Step 1: Expose deterministic prompt rendering**

Add a public helper:

```rust
pub fn local_model_prompt_for_input(input: &LocalModelStewardInput) -> String {
    LocalModelRequest::from_input(input).prompt
}
```

Re-export it from `crates/continuitydb-steward/src/lib.rs`.

- [x] **Step 2: Add a steward unit test**

Add a local-model test proving the helper returns the same prompt text sent to a backend.

### Task 3: CLI Prompt Artifact Wiring

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`

- [x] **Step 1: Add CLI option and options field**

Add `prompt_dir: Option<&'a Path>` to `LocalModelBenchmarkOptions`, add `#[arg(long = "prompt-dir")] prompt_dir: Option<PathBuf>` to `BenchmarkLocalModel`, and pass it through command dispatch.

- [x] **Step 2: Add artifact helper**

Add a `LocalModelPromptArtifact` struct and a helper that creates the directory, writes ordered prompt files, and returns metadata.

- [x] **Step 3: Include prompt artifacts in output**

Add `"prompt_artifacts"` to dry-run and benchmark JSON. It should be `[]` when `--prompt-dir` is absent.

- [x] **Step 4: Verify GREEN**

Run:

```bash
cargo test -p continuitydb-cli cli_benchmark_local_model_prompt_dir_writes_prompt_artifacts --features local-model
```

Expected: PASS.

### Task 4: Documentation and Full Gate

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-cli-local-model-benchmark-prompt-dir.md`

- [x] **Step 1: Update README**

Add:

```markdown
- CLI local model benchmark prompt artifact directory.
```

- [x] **Step 2: Update roadmap**

Add Steward milestone 48:

```markdown
48. Add CLI local-model benchmark prompt artifact directory. Added `benchmark-local-model --prompt-dir` so dry-runs and benchmark runs can write deterministic per-case prompt artifacts and report their fingerprints for reproducible local Steward model trials.
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
git add README.md docs/roadmap.md docs/superpowers/specs/2026-05-20-cli-local-model-benchmark-prompt-dir-design.md docs/superpowers/plans/2026-05-20-cli-local-model-benchmark-prompt-dir.md crates/continuitydb-steward/src/local_model.rs crates/continuitydb-steward/src/lib.rs crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs
git commit -m "feat: add local model benchmark prompt dir"
```
