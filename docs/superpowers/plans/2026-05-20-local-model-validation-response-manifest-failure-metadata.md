# Local Model Validation Response Manifest Failure Metadata Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Preserve best-effort response artifact manifest metadata in local-model validation failure reports.

**Architecture:** Add a local-model validation failure helper for `responses/local-model-responses.manifest.json`. The failure report writer composes manifest, benchmark report, changed-case report, and response manifest metadata independently so missing optional artifacts remain null.

**Tech Stack:** Rust workspace, `continuitydb-cli`, `assert_cmd`, serde JSON, cargo tests with the `local-model` feature.

---

### Task 1: Add response manifest metadata to local-model validation failure reports

**Files:**
- Modify: `crates/continuitydb-cli/tests/cli.rs`
- Modify: `crates/continuitydb-cli/src/main.rs`
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [x] **Step 1: Write the failing acceptance test**

Add `cli_validate_local_model_bundle_failure_report_records_response_manifest_metadata` near the other local-model bundle validation failure report tests. The test must:

- create a real local-model benchmark bundle with `write_real_local_model_bundle`
- mutate `local-model-benchmark.manifest.json` so `benchmark_report_bytes` is wrong
- run `validate-local-model-bundle --artifact-dir <dir> --failure-report-path <failure.json>`
- assert `response_artifact_manifest.manifest_path` equals `responses/local-model-responses.manifest.json`
- assert `manifest_fingerprint` starts with `fnv1a64:`
- assert `manifest_bytes > 0`

- [x] **Step 2: Run focused test to verify RED**

Run:

```sh
cargo test -p continuitydb-cli --features local-model cli_validate_local_model_bundle_failure_report_records_response_manifest_metadata --test cli
```

Expected: fail because `response_artifact_manifest` is currently `null` in local-model validation failure reports.

- [x] **Step 3: Implement best-effort response manifest metadata**

Add a helper in `crates/continuitydb-cli/src/main.rs`:

```rust
#[cfg(feature = "local-model")]
fn local_model_validation_failure_response_artifact_manifest_json(
    artifact_dir: &Path,
) -> serde_json::Value {
    let manifest_path = artifact_dir
        .join("responses")
        .join("local-model-responses.manifest.json");
    let Ok(manifest_text) = std::fs::read_to_string(&manifest_path) else {
        return serde_json::Value::Null;
    };

    serde_json::json!({
        "manifest_path": manifest_path.display().to_string(),
        "manifest_fingerprint": local_model_contract_fingerprint(&manifest_text),
        "manifest_bytes": manifest_text.len(),
    })
}
```

Call it from `write_local_model_bundle_validation_failure_report` and assign the result to `response_artifact_manifest`.

- [x] **Step 4: Run focused test to verify GREEN**

Run:

```sh
cargo test -p continuitydb-cli --features local-model cli_validate_local_model_bundle_failure_report_records_response_manifest_metadata --test cli
```

Expected: pass.

- [x] **Step 5: Update docs**

Add `CLI local model bundle validation response manifest failure metadata` to `README.md` current scope and add the next local-model roadmap milestone in `docs/roadmap.md`.

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
git add README.md crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs docs/roadmap.md docs/superpowers/plans/2026-05-20-local-model-validation-response-manifest-failure-metadata.md docs/superpowers/specs/2026-05-20-local-model-validation-response-manifest-failure-metadata-design.md
git commit -m "feat: report local-model response manifest evidence"
```
