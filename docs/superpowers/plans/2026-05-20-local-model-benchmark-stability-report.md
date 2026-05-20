# Local Model Benchmark Stability Report Implementation Plan

**Goal:** Add repeated-run stability reporting for local Steward model benchmarks.

**Architecture:** Keep normal benchmark runs unchanged. Add a new public `LocalModelBenchmark::run_stability` method and serializable report types. Compute per-case fingerprints from decoded proposal fields that are semantically meaningful and stable, excluding random proposal IDs.

**Tech Stack:** Rust, existing `continuitydb-steward` local-model feature, existing shell-backed local executable test fixture.

### Task 1: Red Tests

**Files:**
- Modify: `crates/continuitydb-steward/src/lib.rs`

- [x] Add a test where two identical repeated outputs report stable.
- [x] Add a test where two passing but different outputs report unstable.
- [x] Verify red because `LocalModelBenchmark::run_stability` does not exist.

### Task 2: Implementation

**Files:**
- Modify: `crates/continuitydb-steward/src/local_model.rs`
- Modify: `crates/continuitydb-steward/src/lib.rs`

- [x] Add `LocalModelBenchmark::run_stability`.
- [x] Add `LocalModelStabilityReport`.
- [x] Add `LocalModelStabilityCaseReport`.
- [x] Fingerprint decoded proposal output by action, rationale, citations, and created time while excluding random proposal IDs.
- [x] Re-export the new report types.
- [x] Verify focused stability tests pass.

### Task 3: Documentation and Gate

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Add: `docs/superpowers/specs/2026-05-20-local-model-benchmark-stability-report-design.md`
- Add: `docs/superpowers/plans/2026-05-20-local-model-benchmark-stability-report.md`

- [x] Update README and roadmap.
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
git commit -m "feat: add local model stability report"
```
