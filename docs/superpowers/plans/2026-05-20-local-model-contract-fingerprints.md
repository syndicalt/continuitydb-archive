# Local Model Contract Fingerprints Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add deterministic schema and grammar fingerprints to local-model CLI contract and dry-run artifacts.

**Architecture:** Implement a CLI-local FNV-1a 64-bit helper for static contract text. Use it in `write_local_model_contract_json()` and `local_model_benchmark_dry_run_json()` without changing benchmark baseline records.

**Tech Stack:** Rust, `serde_json`, existing feature-gated `continuitydb-cli` local-model tests.

---

### Task 1: Add Failing Contract Fingerprint Assertions

**Files:**
- Modify: `crates/continuitydb-cli/tests/cli.rs`

- [x] **Step 1: Assert dry-run contract fingerprints**

In `cli_benchmark_local_model_dry_run_outputs_preflight_without_baseline`, add:

```rust
assert!(json["schema_fingerprint"]
    .as_str()
    .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
assert!(json["grammar_fingerprint"]
    .as_str()
    .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
```

- [x] **Step 2: Assert contract export fingerprints**

In `cli_local_model_contract_writes_schema_and_grammar`, add:

```rust
assert!(json["schema_fingerprint"]
    .as_str()
    .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
assert!(json["grammar_fingerprint"]
    .as_str()
    .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
```

- [x] **Step 3: Run focused tests to verify RED**

Run:

```bash
cargo test -p continuitydb-cli cli_benchmark_local_model_dry_run_outputs_preflight_without_baseline --features local-model
cargo test -p continuitydb-cli cli_local_model_contract_writes_schema_and_grammar --features local-model
```

Expected: both fail because the fingerprint fields are missing.

### Task 2: Implement Contract Fingerprints

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`

- [x] **Step 1: Add deterministic text fingerprint helper**

Add:

```rust
#[cfg(feature = "local-model")]
fn local_model_contract_fingerprint(text: &str) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("fnv1a64:{hash:016x}")
}
```

- [x] **Step 2: Add fingerprints to contract export JSON**

Update `write_local_model_contract_json()` to bind schema and grammar text once, write those values, and return:

```rust
"schema_fingerprint": local_model_contract_fingerprint(schema),
"grammar_fingerprint": local_model_contract_fingerprint(grammar),
```

- [x] **Step 3: Add fingerprints to dry-run JSON**

Update `local_model_benchmark_dry_run_json()` with:

```rust
"schema_fingerprint": local_model_contract_fingerprint(local_model_response_json_schema()),
"grammar_fingerprint": local_model_contract_fingerprint(local_model_response_gbnf_grammar()),
```

- [x] **Step 4: Run focused tests to verify GREEN**

Run:

```bash
cargo test -p continuitydb-cli cli_benchmark_local_model_dry_run_outputs_preflight_without_baseline --features local-model
cargo test -p continuitydb-cli cli_local_model_contract_writes_schema_and_grammar --features local-model
```

Expected: both PASS.

### Task 3: Update Documentation And Verify

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-local-model-contract-fingerprints.md`

- [x] **Step 1: Update README current scope**

Add:

```markdown
- CLI local model contract fingerprints.
```

- [x] **Step 2: Update Steward roadmap**

Add after milestone 39:

```markdown
40. Add CLI local-model contract fingerprints. Added deterministic schema and grammar fingerprints to contract export and benchmark dry-run JSON so operators can connect archived constraint files to preflight artifacts.
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

- [ ] **Step 5: Commit**

Run:

```bash
git add README.md docs/roadmap.md docs/superpowers/specs/2026-05-20-local-model-contract-fingerprints-design.md docs/superpowers/plans/2026-05-20-local-model-contract-fingerprints.md crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs
git commit -m "feat: fingerprint local model contracts"
```
