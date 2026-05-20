# Local Model Report Artifact Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `benchmark-local-model --report-path` so successful local model benchmark and dry-run JSON output can be written to a durable artifact file.

**Architecture:** Extend only the CLI option surface and output path. The benchmark engine and Steward evaluation semantics remain unchanged; the CLI writes the already-created success JSON to a user-selected path before printing it.

**Tech Stack:** Rust, clap, serde_json, existing `continuitydb-cli` local-model tests.

---

### Task 1: RED Tests

**Files:**
- Modify: `crates/continuitydb-cli/tests/cli.rs`

- [x] Add a dry-run test that passes `--report-path`, asserts the report file exists, parses it, and verifies `dry_run = true`, the expected candidate, and no baseline file.
- [x] Add a passing real-run test that passes `--report-path`, asserts the report file exists, parses it, and verifies `passed = true`, `total_cases = 9`, and the same candidate as stdout.
- [x] Run focused tests and verify RED:

```bash
cargo test -p continuitydb-cli cli_benchmark_local_model_report_path --features local-model
```

### Task 2: Implementation

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`

- [x] Add `report_path: Option<PathBuf>` to the `BenchmarkLocalModel` command.
- [x] Keep report artifact writing in command dispatch so benchmark semantics stay unchanged.
- [x] After building the success JSON in command dispatch, write it to `report_path` when present before printing stdout.
- [x] Use pretty JSON for the artifact, matching stdout formatting.
- [x] Run focused tests and verify GREEN:

```bash
cargo test -p continuitydb-cli cli_benchmark_local_model_report_path --features local-model
```

### Task 3: Documentation and Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-local-model-report-artifact.md`

- [x] Add README current-scope bullet for CLI local-model benchmark report artifact output.
- [x] Add roadmap Steward milestone for CLI local-model benchmark report artifact output.
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
git commit -m "feat: add CLI local model report artifact"
```
