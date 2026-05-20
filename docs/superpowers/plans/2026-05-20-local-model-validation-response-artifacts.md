# Local Model Validation Response Artifacts Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Include validated raw response artifact metadata in successful local-model bundle validation output.

**Architecture:** Store the benchmark report's `response_artifacts` projection in `LocalModelBundleValidation` after the existing manifest and response-file validation succeeds. The CLI success JSON then emits the same artifact list that the validator has already checked against the response manifest.

**Tech Stack:** Rust workspace, `continuitydb-cli`, `assert_cmd`, serde JSON, cargo tests with the `local-model` feature.

---

### Task 1: Add response artifacts to successful local-model validation output

**Files:**
- Modify: `crates/continuitydb-cli/tests/cli.rs`
- Modify: `crates/continuitydb-cli/src/main.rs`
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [x] **Step 1: Write the failing acceptance test**

Add `cli_validate_local_model_bundle_accepts_response_artifact_metadata` near the other local-model bundle validation tests. The test must:

- create a real local-model benchmark bundle with `write_real_local_model_bundle`
- run `validate-local-model-bundle --artifact-dir <dir>`
- assert `response_artifacts` is an array with 9 entries
- assert the first entry has `captured: true`
- assert the first entry has a case name
- assert the first entry `response_path` ends with `.response.json`
- assert the first entry `response_fingerprint` starts with `fnv1a64:`
- assert the first entry `response_bytes > 0`

- [x] **Step 2: Run focused test to verify RED**

Run:

```sh
cargo test -p continuitydb-cli --features local-model cli_validate_local_model_bundle_accepts_response_artifact_metadata --test cli
```

Expected: fail because successful local-model validation output currently omits `response_artifacts`.

- [x] **Step 3: Implement response artifact output**

Modify `LocalModelBundleValidation` to include:

```rust
response_artifacts: serde_json::Value,
```

In `validate_local_model_bundle_manifest`, capture:

```rust
let response_artifacts = report["response_artifacts"].clone();
```

Return it in `LocalModelBundleValidation` and emit it from the `ValidateLocalModelBundle` success JSON:

```rust
"response_artifacts": validation.response_artifacts,
```

- [x] **Step 4: Run focused test to verify GREEN**

Run:

```sh
cargo test -p continuitydb-cli --features local-model cli_validate_local_model_bundle_accepts_response_artifact_metadata --test cli
```

Expected: pass.

- [x] **Step 5: Update docs**

Add `CLI local model bundle validation response artifact output` to `README.md` current scope and add the next local-model roadmap milestone in `docs/roadmap.md`.

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

- [x] **Step 7: Commit**

Commit:

```sh
git add README.md crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs docs/roadmap.md docs/superpowers/plans/2026-05-20-local-model-validation-response-artifacts.md docs/superpowers/specs/2026-05-20-local-model-validation-response-artifacts-design.md
git commit -m "feat: report validated local-model response artifacts"
```
