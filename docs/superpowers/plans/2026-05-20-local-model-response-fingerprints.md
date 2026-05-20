# Local Model Response Fingerprints Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Persist lightweight per-case local Steward model response fingerprints in benchmark reports and durable baselines.

**Architecture:** Reuse the raw response capture path added for response artifacts. Convert captured raw responses into serializable fingerprint summaries on `LocalModelBenchmarkReport`, copy them into `LocalModelBenchmarkBaseline`, and expose them in CLI benchmark JSON.

**Tech Stack:** Rust, serde, existing `continuitydb-steward` local-model benchmark tests, existing CLI benchmark JSON tests.

---

### Task 1: RED Tests

**Files:**
- Modify: `crates/continuitydb-steward/src/lib.rs`
- Modify: `crates/continuitydb-cli/tests/cli.rs`

- [x] Add a Steward test proving `LocalModelBenchmarkReport` contains one response fingerprint summary for a passing case.
- [x] Assert `case_name`, `captured`, `response_fingerprint`, and `response_bytes`.
- [x] Assert `LocalModelBenchmarkBaseline::from_report` preserves the response fingerprint summary.
- [x] Add a CLI test assertion that real `benchmark-local-model` JSON includes durable `response_fingerprints` even when `--response-dir` is not used.
- [x] Run focused tests and verify RED:

```bash
cargo test -p continuitydb-steward response_fingerprints --features local-model
cargo test -p continuitydb-cli cli_benchmark_local_model_records_baseline --features local-model
```

### Task 2: Steward Implementation

**Files:**
- Modify: `crates/continuitydb-steward/src/local_model.rs`
- Modify: `crates/continuitydb-steward/src/lib.rs`

- [x] Add `LocalModelResponseFingerprint` with case name, captured flag, optional fingerprint, and byte count.
- [x] Add accessors for the new type.
- [x] Convert `StewardEvaluationCaseResponse` values into response fingerprint summaries.
- [x] Add response fingerprints to `LocalModelBenchmarkReport`.
- [x] Add response fingerprints to `LocalModelBenchmarkBaseline` with serde default for legacy baselines.
- [x] Re-export `LocalModelResponseFingerprint`.
- [x] Run focused Steward test and verify GREEN:

```bash
cargo test -p continuitydb-steward response_fingerprints --features local-model
```

### Task 3: CLI Exposure

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`
- Modify: `crates/continuitydb-cli/tests/cli.rs`

- [x] Include `response_fingerprints` in real benchmark JSON.
- [x] Keep dry-run `response_fingerprints` as an empty list because no model executes.
- [x] Keep `response_artifacts` as file metadata only when `--response-dir` is used.
- [x] Run focused CLI tests and verify GREEN:

```bash
cargo test -p continuitydb-cli cli_benchmark_local_model --features local-model
```

### Task 4: Documentation and Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-local-model-response-fingerprints.md`

- [x] Add README current-scope bullet for durable local-model response fingerprints.
- [x] Add roadmap Steward milestone for durable local-model response fingerprints.
- [x] Run full verification:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

- [x] Commit with message:

```bash
git commit -m "feat: persist local model response fingerprints"
```
