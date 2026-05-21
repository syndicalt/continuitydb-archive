# Local Model Dry-Run Contract Byte Metadata Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add schema and grammar byte-count metadata to local-model dry-run preflight JSON.

**Architecture:** Extend `local_model_benchmark_dry_run_json` with `schema_bytes` and `grammar_bytes` computed from `local_model_response_json_schema()` and `local_model_response_gbnf_grammar()`. Reuse the existing dry-run CLI acceptance test as the public behavior check.

**Tech Stack:** Rust workspace, `continuitydb-cli`, serde JSON, `assert_cmd`, cargo tests with the `local-model` feature.

---

### Task 1: Add dry-run contract byte metadata

**Files:**
- Modify: `crates/continuitydb-cli/tests/cli.rs`
- Modify: `crates/continuitydb-cli/src/main.rs`
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [x] **Step 1: Write the failing dry-run assertions**

Extend `cli_benchmark_local_model_dry_run_outputs_preflight_without_baseline` with:

```rust
assert!(json["schema_bytes"].as_u64().is_some_and(|bytes| bytes > 0));
assert!(json["grammar_bytes"].as_u64().is_some_and(|bytes| bytes > 0));
```

- [x] **Step 2: Run focused test to verify RED**

Run:

```sh
cargo test -p continuitydb-cli --features local-model cli_benchmark_local_model_dry_run_outputs_preflight_without_baseline --test cli
```

Expected: fail because dry-run JSON does not include byte counts yet.

- [x] **Step 3: Implement dry-run byte-count output**

In `local_model_benchmark_dry_run_json`, add:

```rust
"schema_bytes": local_model_response_json_schema().len(),
"grammar_bytes": local_model_response_gbnf_grammar().len(),
```

to the JSON object next to `schema_fingerprint` and `grammar_fingerprint`.

- [x] **Step 4: Run focused test to verify GREEN**

Run:

```sh
cargo test -p continuitydb-cli --features local-model cli_benchmark_local_model_dry_run_outputs_preflight_without_baseline --test cli
```

Expected: pass.

- [x] **Step 5: Update docs**

Add `CLI local model dry-run contract byte metadata` to `README.md` current scope and add roadmap item 112 in `docs/roadmap.md`.

- [x] **Step 6: Run verification gates**

Run:

```sh
cargo fmt --all
cargo fmt --all -- --check
git diff --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
```

- [ ] **Step 7: Commit**

Commit:

```sh
git add README.md crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs docs/roadmap.md docs/superpowers/plans/2026-05-20-local-model-dry-run-contract-byte-metadata.md docs/superpowers/specs/2026-05-20-local-model-dry-run-contract-byte-metadata-design.md
git commit -m "feat: report local-model dry-run contract bytes"
```
