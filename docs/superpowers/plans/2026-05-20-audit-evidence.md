# Audit Evidence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make checkout audit traces expose structured evidence provenance, not only citation locator strings.

**Architecture:** Add `AuditEvidence` metadata to `AuditTrace`, preserving the existing `citations` field as a compact compatibility view. Populate evidence entries directly from each selected `StateCell.evidence` with source, citation locator, confidence, and trust signals.

**Tech Stack:** Rust 2021, `serde`, `continuitydb-core`, `continuitydb-checkout`.

---

### Task 1: Structured Evidence Audit Traces

**Files:**
- Modify: `crates/continuitydb-checkout/src/lib.rs`
- Modify: `crates/continuitydb-cli/tests/cli.rs`
- Modify: `docs/roadmap.md`
- Create: `docs/superpowers/plans/2026-05-20-audit-evidence.md`

- [x] **Step 1: Write failing direct audit test**

Add a test that calls `audit(&cell)` and asserts `trace.evidence[0]` includes the source ID, citation locator, confidence, and trust signals from the cell evidence.

- [x] **Step 2: Write failing checkout JSON test**

Update the CLI smoke test to assert `audit_traces[0].evidence` is present and contains one entry.

- [x] **Step 3: Verify RED**

Run:

```bash
cargo test -p continuitydb-checkout audit_includes_structured_evidence
cargo test -p continuitydb-cli cli_demo_checkout_outputs_metadata_json
```

Expected: FAIL because `AuditTrace` has no `evidence` field.

- [x] **Step 4: Implement `AuditEvidence`**

Add a serializable `AuditEvidence` struct with:

```rust
pub source: String,
pub locator: String,
pub confidence: Confidence,
pub trust: Vec<TrustSignal>,
```

Add `evidence: Vec<AuditEvidence>` to `AuditTrace` and populate it in `audit`.

- [x] **Step 5: Run focused tests**

Run:

```bash
cargo test -p continuitydb-checkout audit
cargo test -p continuitydb-cli
```

Expected: PASS.

- [x] **Step 6: Update roadmap**

Add a checkout/audit milestone noting structured evidence provenance appears in audit traces.

- [x] **Step 7: Run workspace verification**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

Expected: all commands pass.

- [x] **Step 8: Commit**

```bash
git add crates/continuitydb-checkout/src/lib.rs crates/continuitydb-cli/tests/cli.rs docs/roadmap.md docs/superpowers/plans/2026-05-20-audit-evidence.md
git commit -m "feat: audit structured evidence"
```
