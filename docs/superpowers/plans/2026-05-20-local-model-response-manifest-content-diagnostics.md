# Local Model Response Manifest Content Diagnostics Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make local-model response artifact manifest content validation failures identify the mismatched field.

**Architecture:** Keep validation in `continuitydb-cli`. Replace the aggregate content check with ordered field checks that return the same base error plus a deterministic field label.

**Tech Stack:** Rust workspace, `continuitydb-cli`, `assert_cmd`, serde JSON, cargo tests with `local-model` feature.

---

### Task 1: Add field-specific response manifest content diagnostics

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`
- Modify: `crates/continuitydb-cli/tests/cli.rs`
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [x] **Step 1: Write the failing acceptance test**

Update `cli_validate_local_model_bundle_rejects_response_manifest_case_name_mismatch` so stderr must contain:

```text
local model response artifact manifest content mismatch: artifacts
```

- [x] **Step 2: Run focused test to verify RED**

Run:

```sh
cargo test -p continuitydb-cli --features local-model cli_validate_local_model_bundle_rejects_response_manifest_case_name_mismatch --test cli
```

Expected: fail because the current error omits the `artifacts` mismatch label.

- [x] **Step 3: Implement ordered mismatch diagnostics**

Change `validate_local_model_response_artifact_manifest_content` to check `format`, `format_version`, and `artifacts` separately, returning field-specific errors while preserving the existing base message.

- [x] **Step 4: Run focused test to verify GREEN**

Run:

```sh
cargo test -p continuitydb-cli --features local-model cli_validate_local_model_bundle_rejects_response_manifest_case_name_mismatch --test cli
```

Expected: pass.

- [x] **Step 5: Update docs**

Add `CLI local model response artifact manifest content mismatch diagnostics` to `README.md` current scope and add the next local-model roadmap milestone in `docs/roadmap.md`.

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
git add README.md crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs docs/roadmap.md docs/superpowers/plans/2026-05-20-local-model-response-manifest-content-diagnostics.md docs/superpowers/specs/2026-05-20-local-model-response-manifest-content-diagnostics-design.md
git commit -m "feat: report response manifest content mismatches"
```
