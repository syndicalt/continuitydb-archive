# Local Model Latest Baseline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add deterministic lookup for the latest stored local model benchmark baseline for a candidate.

**Architecture:** Build on the existing `LocalModelBenchmarkBaselineStore` contract. A helper function reads append-only baselines, filters by candidate model id and role, and returns the baseline with the newest `recorded_at` timestamp.

**Tech Stack:** Rust 2021, `continuitydb-steward` with the `local-model` feature.

---

### Task 1: RED Latest-Baseline Tests

**Files:**
- Modify: `crates/continuitydb-steward/src/lib.rs`

- [x] **Step 1: Write failing tests**

Add feature-gated tests near the local model baseline tests:
- `latest_local_model_baseline_returns_newest_matching_candidate`
- `latest_local_model_baseline_returns_none_without_candidate_match`

The first test should append baselines for multiple candidates and timestamps to `MemoryLocalModelBenchmarkBaselineStore`, call `latest_local_model_benchmark_baseline(&store, small_model_candidates()[0])`, and assert the returned baseline is the newest Qwen2.5 baseline rather than another candidate.

The second test should append only a different candidate and assert the lookup for `small_model_candidates()[0]` returns `None`.

- [x] **Step 2: Verify RED**

Run:

```bash
cargo test -p continuitydb-steward --features local-model latest_local_model_baseline
```

Expected: compilation fails because `latest_local_model_benchmark_baseline` is not implemented/exported.

### Task 2: Implement Latest-Baseline Lookup

**Files:**
- Modify: `crates/continuitydb-steward/src/local_model.rs`
- Modify: `crates/continuitydb-steward/src/lib.rs`

- [x] **Step 1: Add lookup function**

Add:

```rust
pub fn latest_local_model_benchmark_baseline<S>(
    store: &S,
    candidate: SmallModelCandidate,
) -> Result<Option<LocalModelBenchmarkBaseline>, StewardError>
where
    S: LocalModelBenchmarkBaselineStore,
{
    Ok(store
        .list_baselines()?
        .into_iter()
        .filter(|baseline| baseline.candidate_model_id() == candidate.model_id())
        .filter(|baseline| baseline.candidate_role() == candidate.role())
        .max_by_key(LocalModelBenchmarkBaseline::recorded_at))
}
```

- [x] **Step 2: Export the lookup**

Export `latest_local_model_benchmark_baseline` from `lib.rs` and import it in tests.

- [x] **Step 3: Verify GREEN**

Run:

```bash
cargo test -p continuitydb-steward --features local-model latest_local_model_baseline
```

Expected: both latest-baseline tests pass.

### Task 3: Roadmap and Verification

**Files:**
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-local-model-latest-baseline.md`

- [x] **Step 1: Update roadmap milestone 6**

Update milestone 6 to include latest-baseline lookup for regression gates.

- [x] **Step 2: Mark this plan complete**

Check off completed steps in this plan before commit.

- [x] **Step 3: Verify**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

Expected: all checks pass.
