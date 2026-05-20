# CLI Local Model Candidate Requirements Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add strict CLI enforcement for selected local Steward model candidate runtime requirements.

**Architecture:** Add an `enforce_candidate_requirements` boolean to `benchmark-local-model` and its options struct. Before constructing the runtime config, reject grammar-required candidates when no `--grammar-path` was supplied.

**Tech Stack:** Rust, clap, existing `continuitydb-cli` local-model dry-run tests.

---

### Task 1: Add Failing CLI Enforcement Test

**Files:**
- Modify: `crates/continuitydb-cli/tests/cli.rs`

- [x] **Step 1: Add missing grammar enforcement test**

Add near `cli_benchmark_local_model_dry_run_includes_grammar_path`:

```rust
#[cfg(feature = "local-model")]
#[test]
fn cli_benchmark_local_model_enforces_candidate_grammar_requirement(
) -> Result<(), Box<dyn std::error::Error>> {
    let baseline_path = temp_store_path("continuitydb-cli-local-model-enforced-baseline");

    Command::cargo_bin("continuitydb")?
        .arg("benchmark-local-model")
        .arg("--dry-run")
        .arg("--candidate-defaults")
        .arg("--enforce-candidate-requirements")
        .arg("--candidate")
        .arg("Qwen/Qwen2.5-0.5B-Instruct")
        .arg("--executable")
        .arg("/missing/local-model-runner")
        .arg("--model-path")
        .arg("/models/qwen.gguf")
        .arg("--baseline-path")
        .arg(&baseline_path)
        .assert()
        .failure()
        .stderr(contains("missing --grammar-path"));

    assert!(!baseline_path.exists());
    Ok(())
}
```

- [x] **Step 2: Run focused CLI test to verify RED**

Run:

```bash
cargo test -p continuitydb-cli cli_benchmark_local_model_enforces_candidate_grammar_requirement --features local-model
```

Expected: clap rejects unknown `--enforce-candidate-requirements`.

### Task 2: Implement Candidate Requirement Enforcement

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`

- [x] **Step 1: Add command field**

In `Command::BenchmarkLocalModel`, add:

```rust
/// Reject benchmark configurations that violate selected candidate requirements.
#[arg(long = "enforce-candidate-requirements")]
enforce_candidate_requirements: bool,
```

- [x] **Step 2: Add options field**

In `LocalModelBenchmarkOptions`, add:

```rust
enforce_candidate_requirements: bool,
```

- [x] **Step 3: Pass option through command dispatch**

Pass `enforce_candidate_requirements` into `LocalModelBenchmarkOptions`.

- [x] **Step 4: Add enforcement check**

After resolving the candidate and before building config, add:

```rust
if options.enforce_candidate_requirements
    && candidate.requires_grammar()
    && options.grammar_path.is_none()
{
    return Err(std::io::Error::other(
        "selected local model candidate requires grammar-constrained output; missing --grammar-path",
    )
    .into());
}
```

- [x] **Step 5: Run focused CLI test to verify GREEN**

Run:

```bash
cargo test -p continuitydb-cli cli_benchmark_local_model_enforces_candidate_grammar_requirement --features local-model
```

Expected: PASS.

### Task 3: Update Docs And Verify

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-cli-local-model-candidate-requirements.md`

- [x] **Step 1: Update README current scope**

Add:

```markdown
- CLI local model candidate requirement enforcement.
```

- [x] **Step 2: Update Steward roadmap**

Add after milestone 45:

```markdown
46. Add CLI local-model candidate requirement enforcement. Added `benchmark-local-model --enforce-candidate-requirements` so strict dry-runs and benchmark runs reject grammar-required candidates when no grammar artifact path is supplied.
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
git add README.md docs/roadmap.md docs/superpowers/specs/2026-05-20-cli-local-model-candidate-requirements-design.md docs/superpowers/plans/2026-05-20-cli-local-model-candidate-requirements.md crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs
git commit -m "feat: enforce local model candidate requirements"
```
