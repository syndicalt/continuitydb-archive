# Local Model Contract Export Byte Metadata Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add byte-count metadata to standalone local-model contract export JSON.

**Architecture:** Extend `write_local_model_contract_json` to include `schema_bytes` and `grammar_bytes` computed from the same in-memory schema and grammar strings written to disk. Reuse the existing CLI contract export test as the acceptance surface.

**Tech Stack:** Rust workspace, `continuitydb-cli`, serde JSON, `assert_cmd`, cargo tests with the `local-model` feature.

---

### Task 1: Add byte metadata to standalone contract export

**Files:**
- Modify: `crates/continuitydb-cli/tests/cli.rs`
- Modify: `crates/continuitydb-cli/src/main.rs`
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [x] **Step 1: Write the failing output assertions**

Extend `cli_local_model_contract_writes_schema_and_grammar` with:

```rust
assert_eq!(
    json["schema_bytes"].as_u64(),
    Some(fs::read_to_string(&schema_path)?.len() as u64)
);
assert_eq!(
    json["grammar_bytes"].as_u64(),
    Some(grammar.len() as u64)
);
```

- [x] **Step 2: Run focused test to verify RED**

Run:

```sh
cargo test -p continuitydb-cli --features local-model cli_local_model_contract_writes_schema_and_grammar --test cli
```

Expected: fail because standalone contract export JSON does not include byte counts yet.

- [x] **Step 3: Implement byte-count output**

In `write_local_model_contract_json`, add:

```rust
"schema_bytes": schema.len(),
"grammar_bytes": grammar.len(),
```

to the JSON object returned after writing the files.

- [x] **Step 4: Run focused test to verify GREEN**

Run:

```sh
cargo test -p continuitydb-cli --features local-model cli_local_model_contract_writes_schema_and_grammar --test cli
```

Expected: pass.

- [x] **Step 5: Update docs**

Add `CLI local model contract export byte metadata` to `README.md` current scope and add roadmap item 111 in `docs/roadmap.md`.

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
git add README.md crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs docs/roadmap.md docs/superpowers/plans/2026-05-20-local-model-contract-export-byte-metadata.md docs/superpowers/specs/2026-05-20-local-model-contract-export-byte-metadata-design.md
git commit -m "feat: report local-model contract export bytes"
```
