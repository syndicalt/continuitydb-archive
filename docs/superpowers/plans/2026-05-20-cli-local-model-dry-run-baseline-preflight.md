# CLI Local Model Dry-Run Baseline Preflight Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add read-only compatible baseline preflight metadata to `benchmark-local-model --dry-run --compare-baseline`.

**Architecture:** Compute the same dry-run compatibility metadata already used by the benchmark configuration, read existing JSONL baselines only if the file exists, and filter with the same compatibility dimensions as the real regression gate. Keep this in the CLI layer without changing model execution or the default dry-run JSON shape when comparison is not requested.

**Tech Stack:** Rust, clap, serde_json, assert_cmd CLI tests.

---

### Task 1: CLI Test

**Files:**
- Modify: `crates/continuitydb-cli/tests/cli.rs`

- [x] **Step 1: Write the failing test**

Add a feature-gated Unix CLI test beside the local-model benchmark tests:

```rust
#[cfg(all(feature = "local-model", unix))]
#[test]
fn cli_benchmark_local_model_dry_run_reports_compatible_baseline_preflight(
) -> Result<(), Box<dyn std::error::Error>> {
    let executable_path = temp_store_path("continuitydb-cli-local-model-preflight-runner");
    let baseline_path = temp_store_path("continuitydb-cli-local-model-preflight-baseline");
    let script = r#"#!/usr/bin/env sh
cat >/dev/null
printf '%s\n' '{"proposals":[{"action":{"type":"request_verification","cell_id":null,"request":"Gather additional source evidence."},"rationale":"The evidence is thin, so uncertainty remains.","citations":["continuitydb://evaluation/thin-evidence"]},{"action":{"type":"link_revision","source":"00000000-0000-0000-0000-000000000001","kind":"conflicts_with","target":"00000000-0000-0000-0000-000000000002"},"rationale":"The cited evidence directly contradicts the target claim.","citations":["continuitydb://evaluation/conflict-evidence"]}]}'
"#;
    fs::write(&executable_path, script)?;
    let mut permissions = fs::metadata(&executable_path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&executable_path, permissions)?;

    Command::cargo_bin("continuitydb")?
        .arg("benchmark-local-model")
        .arg("--candidate")
        .arg("Qwen/Qwen2.5-0.5B-Instruct")
        .arg("--executable")
        .arg(&executable_path)
        .arg("--model-path")
        .arg("/models/qwen.gguf")
        .arg("--arg")
        .arg("--temp")
        .arg("--arg")
        .arg("0")
        .arg("--baseline-path")
        .arg(&baseline_path)
        .assert()
        .success();
    let before = fs::read_to_string(&baseline_path)?;

    let output = Command::cargo_bin("continuitydb")?
        .arg("benchmark-local-model")
        .arg("--dry-run")
        .arg("--compare-baseline")
        .arg("--candidate")
        .arg("Qwen/Qwen2.5-0.5B-Instruct")
        .arg("--executable")
        .arg(&executable_path)
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

    assert_eq!(json["baseline_preflight"]["compared"].as_bool(), Some(true));
    assert_eq!(
        json["baseline_preflight"]["compatible_baseline_found"].as_bool(),
        Some(true)
    );
    assert!(json["baseline_preflight"]["previous_recorded_at"].is_string());
    assert_eq!(fs::read_to_string(&baseline_path)?, before);

    fs::remove_file(executable_path)?;
    fs::remove_file(baseline_path)?;
    Ok(())
}
```

- [x] **Step 2: Verify RED**

Run:

```bash
cargo test -p continuitydb-cli cli_benchmark_local_model_dry_run_reports_compatible_baseline_preflight --features local-model
```

Expected: FAIL because `baseline_preflight` is absent.

### Task 2: CLI Implementation

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`

- [x] **Step 1: Import baseline store trait**

Import `LocalModelBenchmarkBaselineStore` so the CLI can list file-backed baselines.

- [x] **Step 2: Add dry-run preflight helper**

Add a helper that reads existing baselines only when `baseline_path.exists()`, and call it only when comparison is requested.

- [x] **Step 3: Filter compatibility**

Filter with candidate, schema version, suite fingerprint, schema fingerprint, grammar fingerprint, prompt fingerprint, and runtime arguments.

- [x] **Step 4: Include output**

Add `"baseline_preflight"` to dry-run JSON only when `--compare-baseline` is supplied.

- [x] **Step 5: Verify GREEN**

Run:

```bash
cargo test -p continuitydb-cli cli_benchmark_local_model_dry_run_reports_compatible_baseline_preflight --features local-model
```

Expected: PASS.

### Task 3: Documentation and Full Gate

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-cli-local-model-dry-run-baseline-preflight.md`

- [x] **Step 1: Update README**

Add:

```markdown
- CLI local model dry-run baseline compatibility preflight.
```

- [x] **Step 2: Update roadmap**

Add Steward milestone 50:

```markdown
50. Add CLI local-model dry-run baseline compatibility preflight. Added read-only `benchmark-local-model --dry-run --compare-baseline` baseline inspection so operators can see whether the current runtime and contract metadata have a compatible previous baseline before executing a model.
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
git add README.md docs/roadmap.md docs/superpowers/specs/2026-05-20-cli-local-model-dry-run-baseline-preflight-design.md docs/superpowers/plans/2026-05-20-cli-local-model-dry-run-baseline-preflight.md crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs
git commit -m "feat: add local model dry-run baseline preflight"
```
