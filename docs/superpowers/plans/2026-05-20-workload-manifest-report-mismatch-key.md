# Workload Manifest Report Mismatch Key Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make workload replay manifest report-content validation identify the mismatched report field.

**Architecture:** Preserve the existing manifest-owned report field list and equality checks. Change the validation error to include the key that mismatched, then cover lookup-plan selectivity drift with a CLI test.

**Tech Stack:** Rust workspace, `continuitydb-cli`, assert_cmd predicates, cargo tests.

---

### Task 1: Add keyed report-content mismatch diagnostics

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`
- Modify: `crates/continuitydb-cli/tests/cli.rs`
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [x] **Step 1: Write the failing CLI test**

Add a test that creates a file-kernel workload artifact bundle, tampers `workload-report.json.lookup_plan.candidate_selectivity_basis_points`, runs `replay-workload --require-manifest`, and expects stderr to contain both `workload artifact manifest report content mismatch` and `lookup_plan`.

- [x] **Step 2: Run focused test to verify RED**

Run:

```sh
cargo test -p continuitydb-cli cli_replay_workload_require_manifest_reports_lookup_plan_content_mismatch_key
```

Expected: failure because the existing error does not identify `lookup_plan`.

- [x] **Step 3: Implement minimal production code**

Update `validate_workload_manifest_report_content` so mismatched keys return `workload artifact manifest report content mismatch: {key}`.

- [x] **Step 4: Run focused test to verify GREEN**

Run the same focused test and confirm it passes.

- [x] **Step 5: Update docs**

Record the keyed workload manifest mismatch diagnostic in `README.md` and `docs/roadmap.md`.

- [x] **Step 6: Run verification gates**

Run:

```sh
cargo fmt --all -- --check
git diff --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
```

- [x] **Step 7: Commit**

Commit:

```sh
git add README.md crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs docs/roadmap.md docs/superpowers/plans/2026-05-20-workload-manifest-report-mismatch-key.md docs/superpowers/specs/2026-05-20-workload-manifest-report-mismatch-key-design.md
git commit -m "feat: report workload manifest mismatch keys"
```
