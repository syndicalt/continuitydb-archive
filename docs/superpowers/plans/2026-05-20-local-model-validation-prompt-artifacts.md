# Local Model Validation Prompt Artifacts Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Validate archived prompt artifacts in local-model benchmark bundles.

**Architecture:** Add a prompt artifact validation helper called from `validate_local_model_bundle_manifest`. It compares manifest/report prompt artifact projections, reads each declared prompt file under the bundle prompt directory, validates byte count and fingerprint, and returns the validated artifact JSON for successful validation output.

**Tech Stack:** Rust workspace, `continuitydb-cli`, `assert_cmd`, serde JSON, cargo tests with the `local-model` feature.

---

### Task 1: Validate prompt artifacts in local-model bundle validation

**Files:**
- Modify: `crates/continuitydb-cli/tests/cli.rs`
- Modify: `crates/continuitydb-cli/src/main.rs`
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [x] **Step 1: Write the failing acceptance test**

Add `cli_validate_local_model_bundle_rejects_tampered_prompt_artifact` near the local-model bundle validation tests. The test must:

- create a dry-run local-model benchmark bundle with `write_dry_run_local_model_bundle`
- read the first `prompt_artifacts[0].prompt_path` from `benchmark-report.json`
- overwrite that prompt file with different same-length content
- run `validate-local-model-bundle --artifact-dir <dir>`
- assert validation fails with `local model prompt artifact fingerprint mismatch`

- [x] **Step 2: Run focused test to verify RED**

Run:

```sh
cargo test -p continuitydb-cli --features local-model cli_validate_local_model_bundle_rejects_tampered_prompt_artifact --test cli
```

Expected: fail because validation currently accepts tampered prompt artifact files.

- [x] **Step 3: Implement prompt artifact validation**

Modify `LocalModelBundleValidation` to include:

```rust
prompt_artifacts: serde_json::Value,
```

Add `validate_local_model_prompt_artifacts(artifact_dir, manifest, benchmark_report)` that:

- returns `Null` if `manifest["prompt_artifacts"]` is null
- rejects when `manifest["prompt_artifacts"] != benchmark_report["prompt_artifacts"]`
- requires the prompt artifacts value to be an array
- for each artifact, requires `prompt_path` under `artifact_dir/prompts`
- reads the prompt file
- compares `prompt_bytes`
- compares `prompt_fingerprint`
- returns the benchmark report `prompt_artifacts` value

Call this helper from `validate_local_model_bundle_manifest` and emit it from successful validation JSON as `prompt_artifacts`.

- [x] **Step 4: Run focused test to verify GREEN**

Run:

```sh
cargo test -p continuitydb-cli --features local-model cli_validate_local_model_bundle_rejects_tampered_prompt_artifact --test cli
```

Expected: pass.

- [x] **Step 5: Update docs**

Add `CLI local model prompt artifact metadata validation` to `README.md` current scope and add the next local-model roadmap milestone in `docs/roadmap.md`.

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
git add README.md crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs docs/roadmap.md docs/superpowers/plans/2026-05-20-local-model-validation-prompt-artifacts.md docs/superpowers/specs/2026-05-20-local-model-validation-prompt-artifacts-design.md
git commit -m "feat: validate local-model prompt artifacts"
```
