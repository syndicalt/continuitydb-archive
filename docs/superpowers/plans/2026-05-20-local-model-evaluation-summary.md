# Local Model Evaluation Summary Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add public deterministic summary metrics for local Steward model evaluation reports and benchmark baselines.

**Architecture:** Introduce a small serializable `StewardEvaluationSummary` value in `continuitydb-steward::local_model`, derive it from existing case reports, and reuse it in benchmark report/baseline accessors and regression comparison.

**Tech Stack:** Rust 2021, `serde`, existing `continuitydb-steward` local-model feature.

---

### Task 1: Evaluation Summary API

**Files:**
- Modify: `crates/continuitydb-steward/src/local_model.rs`
- Modify: `crates/continuitydb-steward/src/lib.rs`
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [x] **Step 1: Write failing tests**

Add feature-gated tests in `crates/continuitydb-steward/src/lib.rs` near existing Steward evaluation tests:

```rust
#[cfg(feature = "local-model")]
#[test]
fn steward_evaluation_summary_counts_passed_and_failed_cases(
) -> Result<(), Box<dyn std::error::Error>> {
    let passing_cell = StateCellId::new();
    let missing_cell = StateCellId::new();
    let response = serde_json::json!({
        "proposals": [{
            "action": {
                "type": "mark_frontier",
                "cell_id": passing_cell,
            },
            "rationale": "The supplied evidence is stale.",
            "citations": ["test://frontier"]
        }]
    })
    .to_string();
    let steward = LocalModelSteward::new(steward()?, StaticLocalModelBackend::new(response));
    let suite = StewardEvaluationSuite::new(vec![
        StewardEvaluationCase::new("passes", created_at(), "find frontier")
            .with_evidence("test://frontier", "Evidence is stale.")
            .expect_action(StewardAction::MarkFrontier {
                cell_id: passing_cell,
            })
            .require_citation("test://frontier"),
        StewardEvaluationCase::new("fails", created_at(), "find other frontier")
            .with_evidence("test://missing", "Other evidence is stale.")
            .expect_action(StewardAction::MarkFrontier {
                cell_id: missing_cell,
            })
            .require_citation("test://missing"),
    ]);

    let summary = suite.evaluate(&steward).summary();

    assert_eq!(summary.total_cases(), 2);
    assert_eq!(summary.passed_cases(), 1);
    assert_eq!(summary.failed_cases(), 1);
    assert_eq!(summary.pass_rate(), 0.5);
    assert!(!summary.passed());
    Ok(())
}

#[cfg(feature = "local-model")]
#[test]
fn local_model_benchmark_baseline_exposes_evaluation_summary(
) -> Result<(), Box<dyn std::error::Error>> {
    let baseline = local_model_empty_baseline(small_model_candidates()[0], created_at())?;

    let summary = baseline.evaluation_summary();

    assert_eq!(summary.total_cases(), 0);
    assert_eq!(summary.passed_cases(), 0);
    assert_eq!(summary.failed_cases(), 0);
    assert_eq!(summary.pass_rate(), 1.0);
    assert!(summary.passed());
    Ok(())
}
```

Update the local-model import list to include `StewardEvaluationSummary`.

- [x] **Step 2: Verify RED**

Run:

```bash
cargo test -p continuitydb-steward steward_evaluation_summary --features local-model
cargo test -p continuitydb-steward local_model_benchmark_baseline_exposes_evaluation_summary --features local-model
```

Expected: compile failure because the summary type and accessors do not exist.

- [x] **Step 3: Implement summary type and accessors**

Add `StewardEvaluationSummary` after `StewardEvaluationReport`, implement `from_report`, accessors, `pass_rate`, and `passed`.

Add:

```rust
pub fn summary(&self) -> StewardEvaluationSummary
```

to `StewardEvaluationReport`.

Add:

```rust
pub fn evaluation_summary(&self) -> StewardEvaluationSummary
```

to `LocalModelBenchmarkReport` and `LocalModelBenchmarkBaseline`.

Update `LocalModelBenchmarkRegression::compare` to call `previous.evaluation_summary().passed_cases()` and `current.evaluation_summary().passed_cases()`.

Re-export `StewardEvaluationSummary` from `crates/continuitydb-steward/src/lib.rs`.

- [x] **Step 4: Verify GREEN**

Run:

```bash
cargo test -p continuitydb-steward steward_evaluation_summary --features local-model
cargo test -p continuitydb-steward local_model_benchmark_baseline_exposes_evaluation_summary --features local-model
```

Expected: tests pass.

- [x] **Step 5: Update docs**

Add README current-scope bullet:

```markdown
- Public local model evaluation summary metrics.
```

Add Steward milestone:

```markdown
30. Add local-model evaluation summary metrics. Implemented serializable deterministic evaluation summaries for reports and baselines so embedders can inspect total, passed, failed, and pass-rate metrics without duplicating regression internals.
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
git add README.md docs/roadmap.md docs/superpowers/specs/2026-05-20-local-model-evaluation-summary-design.md docs/superpowers/plans/2026-05-20-local-model-evaluation-summary.md crates/continuitydb-steward/src/local_model.rs crates/continuitydb-steward/src/lib.rs
git commit -m "feat: summarize local model evaluations"
```
