# Local Model Evaluation Suite Fingerprints Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Persist and compare deterministic local-model evaluation suite fingerprints so benchmark baselines only compare compatible case contracts.

**Architecture:** Add a public fingerprint accessor to `StewardEvaluationSuite`, propagate that value through benchmark reports and baselines, include it in compatible-baseline filtering, and expose it in CLI benchmark JSON. Use an internal deterministic FNV-1a field stream to avoid new dependencies.

**Tech Stack:** Rust, serde, Cargo workspace tests, existing `local-model` feature.

---

### Task 1: Prove Fingerprint Semantics and Baseline Compatibility

**Files:**
- Modify: `crates/continuitydb-steward/src/lib.rs`
- Modify: `crates/continuitydb-cli/tests/cli.rs`

- [x] **Step 1: Add failing steward tests**

Add tests proving deterministic fingerprints, report/baseline persistence, legacy decode, and compatibility filtering:

```rust
#[cfg(feature = "local-model")]
#[test]
fn steward_evaluation_suite_fingerprint_changes_with_case_contract() {
    let base = StewardEvaluationSuite::new(vec![StewardEvaluationCase::new(
        "frontier",
        created_at(),
        "mark frontier",
    )
    .with_evidence("test://frontier", "Evidence is stale.")
    .expect_action(StewardAction::MarkFrontier {
        cell_id: StateCellId::from_u128(7),
    })
    .require_citation("test://frontier")]);
    let same = base.clone();
    let changed = StewardEvaluationSuite::new(vec![StewardEvaluationCase::new(
        "frontier",
        created_at(),
        "mark frontier",
    )
    .with_evidence("test://frontier", "Evidence is stale.")
    .expect_action(StewardAction::MarkFrontier {
        cell_id: StateCellId::from_u128(7),
    })
    .require_citation("test://different")]);

    assert_eq!(base.fingerprint(), same.fingerprint());
    assert_ne!(base.fingerprint(), changed.fingerprint());
    assert!(base.fingerprint().starts_with("fnv1a64:"));
}

#[cfg(feature = "local-model")]
#[test]
fn local_model_benchmark_report_and_baseline_preserve_suite_fingerprint(
) -> Result<(), Box<dyn std::error::Error>> {
    let suite = default_steward_evaluation_suite();
    let expected = suite.fingerprint();
    let benchmark = LocalModelBenchmark::new(
        small_model_candidates()[0],
        LocalExecutableRunner::new(LocalExecutableRunnerConfig::new("llama-cli")),
        suite,
    );

    let report = benchmark.run(steward()?);
    let baseline = LocalModelBenchmarkBaseline::from_report(report.clone(), created_at());

    assert_eq!(report.evaluation_suite_fingerprint(), expected);
    assert_eq!(baseline.evaluation_suite_fingerprint(), expected);
    Ok(())
}

#[cfg(feature = "local-model")]
#[test]
fn local_model_benchmark_baseline_decodes_legacy_json_without_suite_fingerprint(
) -> Result<(), Box<dyn std::error::Error>> {
    let encoded = serde_json::json!({
        "candidate_model_id": "Qwen/Qwen2.5-0.5B-Instruct",
        "candidate_role": "default-feasibility",
        "response_schema_version": LOCAL_MODEL_RESPONSE_SCHEMA_VERSION,
        "runtime": { "executable": "sh", "arguments": [] },
        "evaluation": { "case_reports": [] },
        "recorded_at": created_at(),
    });

    let baseline: LocalModelBenchmarkBaseline = serde_json::from_value(encoded)?;

    assert_eq!(baseline.evaluation_suite_fingerprint(), "");
    Ok(())
}

#[cfg(feature = "local-model")]
#[test]
fn latest_compatible_local_model_baseline_requires_suite_fingerprint(
) -> Result<(), Box<dyn std::error::Error>> {
    let old_compatible = local_model_empty_baseline(
        small_model_candidates()[0],
        Utc.with_ymd_and_hms(2026, 5, 20, 1, 0, 0)
            .single()
            .unwrap_or_else(Utc::now),
    )?;
    let incompatible_suite = local_model_suite_baseline(
        small_model_candidates()[0],
        Utc.with_ymd_and_hms(2026, 5, 20, 3, 0, 0)
            .single()
            .unwrap_or_else(Utc::now),
        StewardEvaluationSuite::new(vec![StewardEvaluationCase::new(
            "different suite",
            created_at(),
            "different task",
        )]),
    )?;
    let current = local_model_empty_baseline(
        small_model_candidates()[0],
        Utc.with_ymd_and_hms(2026, 5, 20, 4, 0, 0)
            .single()
            .unwrap_or_else(Utc::now),
    )?;
    let mut store = MemoryLocalModelBenchmarkBaselineStore::default();
    store.append_baseline(old_compatible.clone())?;
    store.append_baseline(incompatible_suite)?;

    let latest = latest_compatible_local_model_benchmark_baseline(&store, &current)?;

    assert_eq!(latest, Some(old_compatible));
    Ok(())
}
```

Add CLI assertion to `cli_benchmark_local_model_records_baseline`:

```rust
assert!(json["evaluation_suite_fingerprint"]
    .as_str()
    .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
assert_eq!(
    records[0]["evaluation_suite_fingerprint"].as_str(),
    json["evaluation_suite_fingerprint"].as_str()
);
```

- [x] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p continuitydb-steward local_model_benchmark_report_and_baseline_preserve_suite_fingerprint --features local-model
cargo test -p continuitydb-steward latest_compatible_local_model_baseline_requires_suite_fingerprint --features local-model
cargo test -p continuitydb-cli cli_benchmark_local_model_records_baseline --features local-model
```

Expected: FAIL because fingerprint APIs and JSON fields do not exist.

### Task 2: Implement Suite Fingerprints

**Files:**
- Modify: `crates/continuitydb-steward/src/local_model.rs`
- Modify: `crates/continuitydb-steward/src/lib.rs`
- Modify: `crates/continuitydb-cli/src/main.rs`

- [x] **Step 1: Add deterministic field-stream hash helpers**

Add private helpers in `local_model.rs`:

```rust
fn fingerprint_fields(fields: &[String]) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for field in fields {
        for byte in field.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash ^= 0xff;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("fnv1a64:{hash:016x}")
}
```

- [x] **Step 2: Add `StewardEvaluationSuite::fingerprint`**

Build ordered fields from case accessors and expected action JSON strings, then return `fingerprint_fields(&fields)`.

- [x] **Step 3: Propagate report and baseline fields**

Add `evaluation_suite_fingerprint: String` to `LocalModelBenchmarkReport` and `LocalModelBenchmarkBaseline`; add public accessors; set report field from `self.suite.fingerprint()`; set baseline field from report; default missing baseline JSON to empty string.

- [x] **Step 4: Filter compatible baselines by suite fingerprint**

Add:

```rust
.filter(|baseline| {
    baseline.evaluation_suite_fingerprint() == current.evaluation_suite_fingerprint()
})
```

to `latest_compatible_local_model_benchmark_baseline`.

- [x] **Step 5: Expose CLI benchmark JSON field**

Add this to `local_model_benchmark_json`:

```rust
"evaluation_suite_fingerprint": baseline.evaluation_suite_fingerprint(),
```

- [x] **Step 6: Run focused tests to verify they pass**

Run:

```bash
cargo test -p continuitydb-steward steward_evaluation_suite_fingerprint_changes_with_case_contract --features local-model
cargo test -p continuitydb-steward local_model_benchmark_report_and_baseline_preserve_suite_fingerprint --features local-model
cargo test -p continuitydb-steward latest_compatible_local_model_baseline_requires_suite_fingerprint --features local-model
cargo test -p continuitydb-cli cli_benchmark_local_model_records_baseline --features local-model
```

Expected: PASS.

### Task 3: Document and Verify

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-local-model-evaluation-suite-fingerprints.md`

- [x] **Step 1: Update README scope**

Add:

```markdown
- Durable local model evaluation suite fingerprints.
```

- [x] **Step 2: Update roadmap**

Add milestone 36:

```markdown
36. Add local-model evaluation suite fingerprints. Persisted deterministic evaluation-suite fingerprints on benchmark reports and baselines, exposed them in CLI benchmark JSON, and required matching fingerprints for compatible baseline regression gates.
```

- [x] **Step 3: Run full verification gate**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

Expected: all commands pass without warnings or whitespace errors.

- [x] **Step 4: Mark this plan complete and commit**

Run:

```bash
git add README.md docs/roadmap.md docs/superpowers/specs/2026-05-20-local-model-evaluation-suite-fingerprints-design.md docs/superpowers/plans/2026-05-20-local-model-evaluation-suite-fingerprints.md crates/continuitydb-steward/src/local_model.rs crates/continuitydb-steward/src/lib.rs crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs
git commit -m "feat: fingerprint local model evaluation suites"
```
