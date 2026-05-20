# Local Model Response Manifest Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Write a self-describing manifest into `benchmark-local-model --response-dir` directories.

**Architecture:** Extend the existing response artifact writer in the CLI to emit a JSON manifest after per-case response files are written. Keep the manifest as CLI artifact metadata only; durable baselines continue to store response fingerprints, not raw artifact paths.

**Tech Stack:** Rust, serde_json, existing `continuitydb-cli` local-model tests.

---

### Task 1: RED Test

**Files:**
- Modify: `crates/continuitydb-cli/tests/cli.rs`

- [x] Add assertions to `cli_benchmark_local_model_response_dir_writes_response_artifacts` for `response_artifact_manifest`.
- [x] Assert the manifest path exists and is named `local-model-responses.manifest.json`.
- [x] Assert the manifest JSON has `format = continuitydb.local_model.responses`, `format_version = 1`, and nine artifacts.
- [x] Assert the first manifest artifact matches the first response artifact fingerprint and path.
- [x] Run focused test and verify RED:

```bash
cargo test -p continuitydb-cli cli_benchmark_local_model_response_dir --features local-model
```

### Task 2: CLI Implementation

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`

- [x] Add `LocalModelResponseArtifactManifest` with manifest path, fingerprint, and byte count.
- [x] Write `local-model-responses.manifest.json` after response files are written.
- [x] Include the manifest metadata in real benchmark JSON as `response_artifact_manifest`.
- [x] Keep dry-run `response_artifact_manifest` as `null`.
- [x] Run focused CLI test and verify GREEN:

```bash
cargo test -p continuitydb-cli cli_benchmark_local_model_response_dir --features local-model
```

### Task 3: Documentation and Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-local-model-response-manifest.md`

- [x] Add README current-scope bullet for CLI local-model response artifact manifests.
- [x] Add roadmap Steward milestone for CLI local-model response artifact manifests.
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
git commit -m "feat: add local model response artifact manifest"
```
