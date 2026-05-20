# CLI Small Model Candidates Output Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a feature-gated `local-model-candidates` CLI command that exports the fixed small-model Steward candidate registry as JSON.

**Architecture:** Reuse the existing `small_model_candidates()` public API in `continuitydb-steward`. Keep CLI output deterministic, ordered, and read-only; no model execution or registry mutation is introduced.

**Tech Stack:** Rust, clap subcommands, `serde_json`, existing `continuitydb-cli` local-model feature tests.

---

### Task 1: Add Candidate Registry CLI Output

**Files:**
- Modify: `crates/continuitydb-cli/tests/cli.rs`
- Modify: `crates/continuitydb-cli/src/main.rs`

- [x] **Step 1: Write the failing CLI test**

Add this test near the other local-model CLI tests:

```rust
#[cfg(feature = "local-model")]
#[test]
fn cli_local_model_candidates_outputs_fixed_registry() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::cargo_bin("continuitydb")?
        .arg("local-model-candidates")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(
        json["default_candidate"].as_str(),
        Some("Qwen/Qwen2.5-0.5B-Instruct")
    );
    assert_eq!(json["total_candidates"].as_u64(), Some(4));
    assert_eq!(
        json["candidates"][0]["model_id"].as_str(),
        Some("Qwen/Qwen2.5-0.5B-Instruct")
    );
    assert_eq!(
        json["candidates"][0]["role"].as_str(),
        Some("default-feasibility")
    );
    assert!(json["candidates"].as_array().is_some_and(|candidates| {
        candidates.iter().any(|candidate| {
            candidate["model_id"].as_str() == Some("HuggingFaceTB/SmolLM2-360M-Instruct")
                && candidate["role"].as_str() == Some("ultra-small-experimental")
        })
    }));
    Ok(())
}
```

- [x] **Step 2: Run the focused test to verify RED**

Run:

```bash
cargo test -p continuitydb-cli cli_local_model_candidates_outputs_fixed_registry --features local-model
```

Expected: FAIL because `local-model-candidates` is not a known subcommand.

- [x] **Step 3: Add the minimal CLI command**

Add a feature-gated `LocalModelCandidates` variant to `Command`:

```rust
/// Print the fixed local Steward model candidate registry as JSON.
#[cfg(feature = "local-model")]
LocalModelCandidates,
```

Add this match arm in `run()`:

```rust
#[cfg(feature = "local-model")]
Some(Command::LocalModelCandidates) => {
    let output = local_model_candidates_json();
    println!("{}", serde_json::to_string_pretty(&output)?);
}
```

Add this helper near the other local-model JSON helpers:

```rust
#[cfg(feature = "local-model")]
fn local_model_candidates_json() -> serde_json::Value {
    let candidates = small_model_candidates();
    serde_json::json!({
        "default_candidate": candidates.first().map(SmallModelCandidate::model_id),
        "total_candidates": candidates.len(),
        "candidates": candidates
            .iter()
            .map(|candidate| {
                serde_json::json!({
                    "model_id": candidate.model_id(),
                    "role": candidate.role(),
                })
            })
            .collect::<Vec<_>>(),
    })
}
```

- [x] **Step 4: Run the focused test to verify GREEN**

Run:

```bash
cargo test -p continuitydb-cli cli_local_model_candidates_outputs_fixed_registry --features local-model
```

Expected: PASS.

### Task 2: Update Documentation And Verify

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-cli-small-model-candidates-output.md`

- [x] **Step 1: Update README current scope**

Add:

```markdown
- CLI small local Steward model candidate registry output.
```

- [x] **Step 2: Update Steward roadmap**

Add this milestone after CLI local-model evaluation suite fingerprint output:

```markdown
38. Add CLI small-model candidate registry output. Exposed the fixed local Steward candidate registry through `local-model-candidates` JSON so operators can inspect supported model IDs and roles before benchmark runs.
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
git add README.md docs/roadmap.md docs/superpowers/specs/2026-05-20-cli-small-model-candidates-output-design.md docs/superpowers/plans/2026-05-20-cli-small-model-candidates-output.md crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs
git commit -m "feat: expose local model candidates in cli"
```
