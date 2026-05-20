# CLI Local Model Benchmark Contract Directory Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `benchmark-local-model --contract-dir <DIR>` so benchmark dry-runs and real runs can materialize local Steward schema/grammar artifacts and use the generated grammar path automatically.

**Architecture:** Keep contract generation inside the CLI command boundary by reusing existing schema, grammar, and fingerprint helpers. Resolve benchmark runtime grammar as explicit `--grammar-path` first, generated contract grammar second, and absent otherwise.

**Tech Stack:** Rust, clap, serde_json, assert_cmd CLI smoke tests.

---

### Task 1: Contract Directory CLI Test

**Files:**
- Modify: `crates/continuitydb-cli/tests/cli.rs`

- [x] **Step 1: Write the failing test**

Add a feature-gated CLI test beside the existing grammar/enforcement dry-run tests:

```rust
#[cfg(feature = "local-model")]
#[test]
fn cli_benchmark_local_model_contract_dir_writes_artifacts_and_supplies_grammar(
) -> Result<(), Box<dyn std::error::Error>> {
    let baseline_path = temp_store_path("continuitydb-cli-local-model-contract-dir-baseline");
    let contract_dir = std::env::temp_dir().join(format!(
        "continuitydb-cli-local-model-contract-dir-{}",
        std::process::id()
    ));
    if contract_dir.exists() {
        fs::remove_dir_all(&contract_dir)?;
    }

    let output = Command::cargo_bin("continuitydb")?
        .arg("benchmark-local-model")
        .arg("--dry-run")
        .arg("--candidate-defaults")
        .arg("--enforce-candidate-requirements")
        .arg("--contract-dir")
        .arg(&contract_dir)
        .arg("--candidate")
        .arg("Qwen/Qwen2.5-0.5B-Instruct")
        .arg("--executable")
        .arg("/missing/local-model-runner")
        .arg("--model-path")
        .arg("/models/qwen.gguf")
        .arg("--baseline-path")
        .arg(&baseline_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;
    let schema_path = contract_dir.join("local-model-response.schema.json");
    let grammar_path = contract_dir.join("local-model-response.gbnf");

    assert!(schema_path.exists());
    assert!(grammar_path.exists());
    assert_eq!(
        json["contract_artifacts"]["schema_path"].as_str(),
        Some(schema_path.display().to_string().as_str())
    );
    assert_eq!(
        json["contract_artifacts"]["grammar_path"].as_str(),
        Some(grammar_path.display().to_string().as_str())
    );
    assert!(json["contract_artifacts"]["schema_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
    assert!(json["contract_artifacts"]["grammar_fingerprint"]
        .as_str()
        .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
    assert_eq!(
        json["runtime"]["arguments"][8].as_str(),
        Some("--grammar-file")
    );
    assert_eq!(
        json["runtime"]["arguments"][9].as_str(),
        Some(grammar_path.display().to_string().as_str())
    );
    assert!(!baseline_path.exists());

    fs::remove_dir_all(contract_dir)?;
    Ok(())
}
```

- [x] **Step 2: Verify RED**

Run:

```bash
cargo test -p continuitydb-cli cli_benchmark_local_model_contract_dir_writes_artifacts_and_supplies_grammar --features local-model
```

Expected: FAIL because `--contract-dir` is not accepted.

### Task 2: Implement Contract Directory Wiring

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`

- [x] **Step 1: Add CLI option and options field**

Add `contract_dir: Option<&'a Path>` to `LocalModelBenchmarkOptions`, add `#[arg(long = "contract-dir")] contract_dir: Option<PathBuf>` to `BenchmarkLocalModel`, and pass it through command dispatch.

- [x] **Step 2: Add artifact helper**

Add a small helper that writes deterministic artifact filenames under the supplied directory and returns JSON-friendly metadata:

```rust
#[cfg(feature = "local-model")]
struct LocalModelContractArtifacts {
    schema_path: PathBuf,
    grammar_path: PathBuf,
    schema_fingerprint: String,
    grammar_fingerprint: String,
}
```

- [x] **Step 3: Use generated grammar as fallback**

In `benchmark_local_model_json`, materialize contract artifacts before requirement enforcement. Resolve runtime grammar path as:

```rust
let effective_grammar_path = options
    .grammar_path
    .or_else(|| contract_artifacts.as_ref().map(|artifacts| artifacts.grammar_path.as_path()));
```

Use `effective_grammar_path` for requirement enforcement and runtime arguments.

- [x] **Step 4: Include metadata in output**

Add `"contract_artifacts"` to dry-run and benchmark JSON. It should be `null` when no `--contract-dir` was supplied and an object when it was supplied.

- [x] **Step 5: Verify GREEN**

Run:

```bash
cargo test -p continuitydb-cli cli_benchmark_local_model_contract_dir_writes_artifacts_and_supplies_grammar --features local-model
```

Expected: PASS.

### Task 3: Documentation and Full Gate

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-cli-local-model-benchmark-contract-dir.md`

- [x] **Step 1: Update README**

Add:

```markdown
- CLI local model benchmark contract artifact directory.
```

- [x] **Step 2: Update roadmap**

Add Steward milestone 47:

```markdown
47. Add CLI local-model benchmark contract artifact directory. Added `benchmark-local-model --contract-dir` so dry-runs and benchmark runs can materialize schema and grammar artifacts, report their fingerprints, and use the generated grammar path for strict grammar-required candidates.
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
git add README.md docs/roadmap.md docs/superpowers/specs/2026-05-20-cli-local-model-benchmark-contract-dir-design.md docs/superpowers/plans/2026-05-20-cli-local-model-benchmark-contract-dir.md crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs
git commit -m "feat: add local model benchmark contract dir"
```
