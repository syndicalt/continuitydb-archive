# CLI Local Model Grammar Path Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add first-class `benchmark-local-model --grammar-path` support for grammar-constrained local Steward benchmark runs.

**Architecture:** Extend the CLI command and options struct with an optional grammar path. Append `--grammar-file <path>` to the local executable runner config before explicit repeated `--arg` values.

**Tech Stack:** Rust, clap, existing `continuitydb-cli` local-model dry-run tests.

---

### Task 1: Add Failing CLI Dry-Run Test

**Files:**
- Modify: `crates/continuitydb-cli/tests/cli.rs`

- [x] **Step 1: Add grammar path dry-run test**

Add near `cli_benchmark_local_model_dry_run_uses_candidate_defaults`:

```rust
#[cfg(feature = "local-model")]
#[test]
fn cli_benchmark_local_model_dry_run_includes_grammar_path(
) -> Result<(), Box<dyn std::error::Error>> {
    let baseline_path = temp_store_path("continuitydb-cli-local-model-grammar-baseline");
    let grammar_path = temp_store_path("continuitydb-cli-local-model-response-grammar");

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
        .arg("--grammar-path")
        .arg(&grammar_path)
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

    assert_eq!(json["runtime"]["arguments"][6].as_str(), Some("--prompt"));
    assert_eq!(json["runtime"]["arguments"][7].as_str(), Some("-"));
    assert_eq!(json["runtime"]["arguments"][8].as_str(), Some("--grammar-file"));
    assert_eq!(
        json["runtime"]["arguments"][9].as_str(),
        Some(grammar_path.display().to_string().as_str())
    );
    assert_eq!(json["runtime"]["arguments"][10].as_str(), Some("--threads"));
    assert_eq!(json["runtime"]["arguments"][11].as_str(), Some("2"));
    assert!(!baseline_path.exists());
    Ok(())
}
```

- [x] **Step 2: Run focused CLI test to verify RED**

Run:

```bash
cargo test -p continuitydb-cli cli_benchmark_local_model_dry_run_includes_grammar_path --features local-model
```

Expected: clap rejects unknown `--grammar-path`.

### Task 2: Implement CLI Grammar Path

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`

- [x] **Step 1: Add command field**

In `Command::BenchmarkLocalModel`, add:

```rust
/// GBNF grammar path passed to the executable as `--grammar-file <path>`.
#[arg(long = "grammar-path")]
grammar_path: Option<PathBuf>,
```

- [x] **Step 2: Add options field**

In `LocalModelBenchmarkOptions`, add:

```rust
grammar_path: Option<&'a Path>,
```

- [x] **Step 3: Pass option through command dispatch**

Pass `grammar_path.as_deref()` into `LocalModelBenchmarkOptions`.

- [x] **Step 4: Append grammar-file arguments before explicit args**

After base config construction and before the existing explicit argument loop, add:

```rust
if let Some(grammar_path) = options.grammar_path {
    config = config
        .with_argument("--grammar-file")
        .with_argument(grammar_path);
}
```

- [x] **Step 5: Run focused CLI test to verify GREEN**

Run:

```bash
cargo test -p continuitydb-cli cli_benchmark_local_model_dry_run_includes_grammar_path --features local-model
```

Expected: PASS.

### Task 3: Update Docs And Verify

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-cli-local-model-grammar-path.md`

- [x] **Step 1: Update README current scope**

Add:

```markdown
- CLI local model benchmark grammar path support.
```

- [x] **Step 2: Update Steward roadmap**

Add after milestone 44:

```markdown
45. Add CLI local-model benchmark grammar path support. Added `benchmark-local-model --grammar-path` so dry-runs and benchmark runs can pass generated GBNF grammar artifacts as first-class runtime arguments.
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
git add README.md docs/roadmap.md docs/superpowers/specs/2026-05-20-cli-local-model-grammar-path-design.md docs/superpowers/plans/2026-05-20-cli-local-model-grammar-path.md crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs
git commit -m "feat: add local model grammar path"
```
