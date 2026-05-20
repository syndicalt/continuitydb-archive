# Public Compatible Local Model Baseline Lookup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Expose the local-model compatible-baseline lookup as a public embeddable API.

**Architecture:** Reuse the existing internal compatibility implementation that powers regression recording. Add public tests through the crate re-export so external embedders get the same strict candidate, schema, and runtime matching semantics.

**Tech Stack:** Rust 2021, `continuitydb-steward`, feature-gated `local-model` APIs, existing in-memory baseline store tests.

---

### Task 1: Public Compatible Baseline Lookup

**Files:**
- Modify: `crates/continuitydb-steward/src/lib.rs`
- Modify: `crates/continuitydb-steward/src/local_model.rs`
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [x] **Step 1: Write failing tests**

Add two feature-gated tests in `crates/continuitydb-steward/src/lib.rs` near the existing latest baseline tests:

```rust
#[cfg(feature = "local-model")]
#[test]
fn latest_compatible_local_model_baseline_matches_runtime_and_schema(
) -> Result<(), Box<dyn std::error::Error>> {
    let old_compatible = local_model_empty_baseline(
        small_model_candidates()[0],
        Utc.with_ymd_and_hms(2026, 5, 20, 1, 0, 0)
            .single()
            .unwrap_or_else(Utc::now),
    )?;
    let incompatible_runtime = local_model_runtime_baseline(
        small_model_candidates()[0],
        Utc.with_ymd_and_hms(2026, 5, 20, 3, 0, 0)
            .single()
            .unwrap_or_else(Utc::now),
        "--different-runtime",
    )?;
    let new_compatible = local_model_empty_baseline(
        small_model_candidates()[0],
        Utc.with_ymd_and_hms(2026, 5, 20, 2, 0, 0)
            .single()
            .unwrap_or_else(Utc::now),
    )?;
    let current = local_model_empty_baseline(
        small_model_candidates()[0],
        Utc.with_ymd_and_hms(2026, 5, 20, 4, 0, 0)
            .single()
            .unwrap_or_else(Utc::now),
    )?;
    let mut store = MemoryLocalModelBenchmarkBaselineStore::default();
    store.append_baseline(old_compatible)?;
    store.append_baseline(incompatible_runtime)?;
    store.append_baseline(new_compatible.clone())?;

    let latest = latest_compatible_local_model_benchmark_baseline(&store, &current)?;

    assert_eq!(latest, Some(new_compatible));
    Ok(())
}

#[cfg(feature = "local-model")]
#[test]
fn latest_compatible_local_model_baseline_returns_none_without_compatible_runtime(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut store = MemoryLocalModelBenchmarkBaselineStore::default();
    store.append_baseline(local_model_runtime_baseline(
        small_model_candidates()[0],
        created_at(),
        "--different-runtime",
    )?)?;
    let current = local_model_empty_baseline(small_model_candidates()[0], created_at())?;

    let latest = latest_compatible_local_model_benchmark_baseline(&store, &current)?;

    assert_eq!(latest, None);
    Ok(())
}
```

Add this helper near `local_model_empty_baseline`:

```rust
#[cfg(feature = "local-model")]
fn local_model_runtime_baseline(
    candidate: SmallModelCandidate,
    recorded_at: chrono::DateTime<Utc>,
    runtime_argument: &str,
) -> Result<LocalModelBenchmarkBaseline, StewardError> {
    Ok(LocalModelBenchmarkBaseline::from_report(
        LocalModelBenchmark::new(
            candidate,
            LocalExecutableRunner::new(
                LocalExecutableRunnerConfig::new("sh").with_argument(runtime_argument),
            ),
            StewardEvaluationSuite::new(Vec::new()),
        )
        .run(steward()?),
        recorded_at,
    ))
}
```

- [x] **Step 2: Verify RED**

Run:

```bash
cargo test -p continuitydb-steward latest_compatible_local_model_baseline --features local-model
```

Expected: compile failure because `latest_compatible_local_model_benchmark_baseline` is not exported/public.

- [x] **Step 3: Implement public API**

In `crates/continuitydb-steward/src/local_model.rs`, make the existing helper public and document it:

```rust
/// Returns the newest stored benchmark baseline compatible with the supplied current baseline.
pub fn latest_compatible_local_model_benchmark_baseline<S>(
    store: &S,
    current: &LocalModelBenchmarkBaseline,
) -> Result<Option<LocalModelBenchmarkBaseline>, StewardError>
where
    S: LocalModelBenchmarkBaselineStore,
{
    Ok(store
        .list_baselines()?
        .into_iter()
        .filter(|baseline| baseline.candidate_model_id() == current.candidate_model_id())
        .filter(|baseline| baseline.candidate_role() == current.candidate_role())
        .filter(|baseline| baseline.response_schema_version() == current.response_schema_version())
        .filter(|baseline| baseline.runtime() == current.runtime())
        .max_by_key(LocalModelBenchmarkBaseline::recorded_at))
}
```

In `crates/continuitydb-steward/src/lib.rs`, add it to the public re-export list and the test import list.

- [x] **Step 4: Verify GREEN**

Run:

```bash
cargo test -p continuitydb-steward latest_compatible_local_model_baseline --features local-model
```

Expected: both tests pass.

- [x] **Step 5: Update docs**

Add current scope and roadmap bullets:

```markdown
- Public compatible local model benchmark baseline lookup for embedders.
```

```markdown
29. Add public compatible local-model baseline lookup. Implemented a feature-gated API that returns the newest stored baseline matching candidate identity, response schema version, and runtime manifest without recording a new benchmark run.
```

- [x] **Step 6: Run full gate**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

Expected: all commands pass.

- [x] **Step 7: Commit**

Run:

```bash
git add README.md docs/roadmap.md docs/superpowers/specs/2026-05-20-public-compatible-local-model-baseline-lookup-design.md docs/superpowers/plans/2026-05-20-public-compatible-local-model-baseline-lookup.md crates/continuitydb-steward/src/local_model.rs crates/continuitydb-steward/src/lib.rs
git commit -m "feat: expose compatible local model baseline lookup"
```
