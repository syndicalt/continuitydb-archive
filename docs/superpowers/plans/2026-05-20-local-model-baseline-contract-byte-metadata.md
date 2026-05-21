# Local Model Baseline Contract Byte Metadata Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Persist schema and grammar byte-count metadata in local-model benchmark baselines and real benchmark JSON.

**Architecture:** Extend the Steward local-model report and durable baseline structs with byte-count fields computed from the canonical in-memory response schema and GBNF grammar strings. The CLI already builds successful benchmark output from `LocalModelBenchmarkBaseline`, so adding baseline accessors and JSON fields makes the byte evidence visible in stdout and durable JSONL records without changing compatibility semantics.

**Tech Stack:** Rust workspace, `continuitydb-steward`, `continuitydb-cli`, serde JSON, `assert_cmd`, cargo tests with the `local-model` feature.

---

### Task 1: Persist local-model baseline contract bytes

**Files:**
- Modify: `crates/continuitydb-cli/tests/cli.rs`
- Modify: `crates/continuitydb-steward/src/local_model.rs`
- Modify: `crates/continuitydb-cli/src/main.rs`
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [x] **Step 1: Write the failing baseline assertions**

Extend `cli_benchmark_local_model_records_baseline` after the existing `grammar_fingerprint` assertions with:

```rust
assert!(json["schema_bytes"].as_u64().is_some_and(|bytes| bytes > 0));
assert!(json["grammar_bytes"].as_u64().is_some_and(|bytes| bytes > 0));
assert_eq!(records[0]["schema_bytes"], json["schema_bytes"]);
assert_eq!(records[0]["grammar_bytes"], json["grammar_bytes"]);
```

- [x] **Step 2: Run focused test to verify RED**

Run:

```sh
cargo test -p continuitydb-cli --features local-model cli_benchmark_local_model_records_baseline --test cli
```

Expected: fail because successful benchmark JSON and durable baseline records do not include contract byte counts yet.

- [x] **Step 3: Add Steward report and baseline byte fields**

In `LocalModelBenchmarkReport`, add:

```rust
pub schema_bytes: usize,
pub grammar_bytes: usize,
```

In `LocalModelBenchmark::report_from_evaluation`, compute the contract strings once and set:

```rust
let schema = local_model_response_json_schema();
let grammar = local_model_response_gbnf_grammar();
```

Then use:

```rust
schema_fingerprint: fingerprint_text(schema),
grammar_fingerprint: fingerprint_text(grammar),
schema_bytes: schema.len(),
grammar_bytes: grammar.len(),
```

In `LocalModelBenchmarkBaseline`, add serde-defaulted fields:

```rust
#[serde(default)]
schema_bytes: usize,
#[serde(default)]
grammar_bytes: usize,
```

Copy the report fields in `from_report`, and add accessors:

```rust
pub fn schema_bytes(&self) -> usize {
    self.schema_bytes
}

pub fn grammar_bytes(&self) -> usize {
    self.grammar_bytes
}
```

- [x] **Step 4: Emit benchmark JSON byte fields**

In `local_model_benchmark_json`, add top-level fields next to the contract fingerprints:

```rust
"schema_bytes": baseline.schema_bytes(),
"grammar_bytes": baseline.grammar_bytes(),
```

- [x] **Step 5: Run focused test to verify GREEN**

Run:

```sh
cargo test -p continuitydb-cli --features local-model cli_benchmark_local_model_records_baseline --test cli
```

Expected: pass.

- [x] **Step 6: Update docs**

Add `Durable local model contract byte metadata` to `README.md` current scope after the durable contract fingerprint item. Add the same milestone to the Steward roadmap after the durable contract fingerprint milestone.

- [x] **Step 7: Run verification gates**

Run:

```sh
cargo fmt --all
cargo fmt --all -- --check
git diff --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
```

- [ ] **Step 8: Commit**

Commit:

```sh
git add README.md crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs crates/continuitydb-steward/src/local_model.rs docs/roadmap.md docs/superpowers/plans/2026-05-20-local-model-baseline-contract-byte-metadata.md docs/superpowers/specs/2026-05-20-local-model-baseline-contract-byte-metadata-design.md
git commit -m "feat: persist local-model contract bytes"
```
