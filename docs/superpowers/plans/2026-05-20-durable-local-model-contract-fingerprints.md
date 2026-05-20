# Durable Local Model Contract Fingerprints Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Persist local-model schema and grammar fingerprints on benchmark reports and baselines, and require them for compatible regression comparisons.

**Architecture:** Add two string fields to `LocalModelBenchmarkReport` and `LocalModelBenchmarkBaseline`. Populate report fields during benchmark execution with deterministic FNV-1a fingerprints of the local model schema and grammar text, copy them into baselines, expose public accessors, and filter compatible baselines by both fingerprints.

**Tech Stack:** Rust, serde, existing `continuitydb-steward` local-model feature tests, existing `continuitydb-cli` benchmark JSON tests.

---

### Task 1: Add Failing Steward Tests

**Files:**
- Modify: `crates/continuitydb-steward/src/lib.rs`

- [x] **Step 1: Add report and baseline preservation test**

Add a feature-gated test near the suite fingerprint test:

```rust
#[cfg(feature = "local-model")]
#[test]
fn local_model_benchmark_report_and_baseline_preserve_contract_fingerprints(
) -> Result<(), Box<dyn std::error::Error>> {
    let benchmark = LocalModelBenchmark::new(
        small_model_candidates()[0],
        LocalExecutableRunner::new(LocalExecutableRunnerConfig::new("llama-cli")),
        StewardEvaluationSuite::new(Vec::new()),
    );

    let report = benchmark.run(steward()?);
    let baseline = LocalModelBenchmarkBaseline::from_report(report.clone(), created_at());

    assert!(report.schema_fingerprint().starts_with("fnv1a64:"));
    assert!(report.grammar_fingerprint().starts_with("fnv1a64:"));
    assert_eq!(baseline.schema_fingerprint(), report.schema_fingerprint());
    assert_eq!(baseline.grammar_fingerprint(), report.grammar_fingerprint());
    Ok(())
}
```

- [x] **Step 2: Add legacy decode test**

Add:

```rust
#[cfg(feature = "local-model")]
#[test]
fn local_model_benchmark_baseline_decodes_legacy_json_without_contract_fingerprints(
) -> Result<(), Box<dyn std::error::Error>> {
    let encoded = serde_json::json!({
        "candidate_model_id": "Qwen/Qwen2.5-0.5B-Instruct",
        "candidate_role": "default-feasibility",
        "response_schema_version": LOCAL_MODEL_RESPONSE_SCHEMA_VERSION,
        "evaluation_suite_fingerprint": default_steward_evaluation_suite().fingerprint(),
        "runtime": { "executable": "sh", "arguments": [] },
        "evaluation": { "case_reports": [] },
        "recorded_at": created_at(),
    });

    let baseline: LocalModelBenchmarkBaseline = serde_json::from_value(encoded)?;

    assert_eq!(baseline.schema_fingerprint(), "");
    assert_eq!(baseline.grammar_fingerprint(), "");
    Ok(())
}
```

- [x] **Step 3: Add compatibility filtering test**

Add:

```rust
#[cfg(feature = "local-model")]
#[test]
fn latest_compatible_local_model_baseline_requires_contract_fingerprints(
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
    incompatible_json["schema_fingerprint"] = serde_json::json!("fnv1a64:0000000000000000");
    let incompatible_contract: LocalModelBenchmarkBaseline =
        serde_json::from_value(incompatible_json)?;
    let current = local_model_empty_baseline(
        small_model_candidates()[0],
        Utc.with_ymd_and_hms(2026, 5, 20, 3, 0, 0)
            .single()
            .unwrap_or_else(Utc::now),
    )?;
    let mut store = MemoryLocalModelBenchmarkBaselineStore::default();
    store.append_baseline(old_compatible.clone())?;
    store.append_baseline(incompatible_contract)?;

    let latest = latest_compatible_local_model_benchmark_baseline(&store, &current)?;

    assert_eq!(latest, Some(old_compatible));
    Ok(())
}
```

- [x] **Step 4: Run steward focused tests to verify RED**

Run:

```bash
cargo test -p continuitydb-steward contract_fingerprint --features local-model
cargo test -p continuitydb-steward latest_compatible_local_model_baseline_requires_contract_fingerprints --features local-model
```

Expected: compile failures because report/baseline accessors do not exist and compatibility does not yet filter by contract fingerprints.

### Task 2: Implement Durable Contract Fingerprints

**Files:**
- Modify: `crates/continuitydb-steward/src/local_model.rs`

- [x] **Step 1: Add contract fingerprint helper**

Add:

```rust
fn fingerprint_text(text: &str) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("fnv1a64:{hash:016x}")
}
```

- [x] **Step 2: Add report fields and accessors**

Add fields to `LocalModelBenchmarkReport`:

```rust
schema_fingerprint: String,
grammar_fingerprint: String,
```

Populate them in `LocalModelBenchmark::run()`:

```rust
schema_fingerprint: fingerprint_text(local_model_response_json_schema()),
grammar_fingerprint: fingerprint_text(local_model_response_gbnf_grammar()),
```

Add public accessors:

```rust
pub fn schema_fingerprint(&self) -> &str { &self.schema_fingerprint }
pub fn grammar_fingerprint(&self) -> &str { &self.grammar_fingerprint }
```

- [x] **Step 3: Add baseline fields and accessors**

Add serde-defaulted fields to `LocalModelBenchmarkBaseline`:

```rust
#[serde(default)]
schema_fingerprint: String,
#[serde(default)]
grammar_fingerprint: String,
```

Copy from report in `from_report()`, then add matching public accessors.

- [x] **Step 4: Require fingerprints for compatible baselines**

Update `latest_compatible_local_model_benchmark_baseline()` with filters:

```rust
.filter(|baseline| baseline.schema_fingerprint() == current.schema_fingerprint())
.filter(|baseline| baseline.grammar_fingerprint() == current.grammar_fingerprint())
```

- [x] **Step 5: Run steward focused tests to verify GREEN**

Run:

```bash
cargo test -p continuitydb-steward contract_fingerprint --features local-model
cargo test -p continuitydb-steward latest_compatible_local_model_baseline_requires_contract_fingerprints --features local-model
```

Expected: PASS.

### Task 3: Expose In CLI Benchmark JSON And Verify

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`
- Modify: `crates/continuitydb-cli/tests/cli.rs`
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-durable-local-model-contract-fingerprints.md`

- [x] **Step 1: Add CLI benchmark assertions**

In `cli_benchmark_local_model_records_baseline`, assert JSON and stored record contract fingerprints start with `fnv1a64:` and match.

- [x] **Step 2: Add CLI benchmark JSON fields**

In `local_model_benchmark_json()`, add:

```rust
"schema_fingerprint": baseline.schema_fingerprint(),
"grammar_fingerprint": baseline.grammar_fingerprint(),
```

- [x] **Step 3: Update README current scope**

Add:

```markdown
- Durable local model contract fingerprints.
```

- [x] **Step 4: Update Steward roadmap**

Add after milestone 40:

```markdown
41. Add durable local-model contract fingerprints. Persisted schema and grammar fingerprints on benchmark reports and baselines, exposed them in CLI benchmark JSON, and required matching fingerprints for compatible baseline regression gates.
```

- [x] **Step 5: Mark this plan complete**

Check off completed steps in this plan before commit.

- [x] **Step 6: Run the full verification gate**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

Expected: all commands exit 0.

- [x] **Step 7: Commit**

Run:

```bash
git add README.md docs/roadmap.md docs/superpowers/specs/2026-05-20-durable-local-model-contract-fingerprints-design.md docs/superpowers/plans/2026-05-20-durable-local-model-contract-fingerprints.md crates/continuitydb-steward/src/local_model.rs crates/continuitydb-steward/src/lib.rs crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs
git commit -m "feat: persist local model contract fingerprints"
```
