# Audit Dependencies Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make checkout audit traces include StateCell dependency and causality links, not just citations.

**Architecture:** Extend `AuditTrace` with serializable `AuditDependency` entries derived from each selected `StateCell.dependencies`. Preserve existing citation behavior and keep audit computation deterministic and kernel-independent.

**Tech Stack:** Rust 2021, `serde`, `continuitydb-core`, `continuitydb-checkout`.

---

### Task 1: Dependency-Aware Audit Traces

**Files:**
- Modify: `crates/continuitydb-checkout/src/lib.rs`
- Modify: `crates/continuitydb-cli/tests/cli.rs`
- Modify: `docs/roadmap.md`
- Create: `docs/superpowers/plans/2026-05-20-audit-dependencies.md`

- [x] **Step 1: Write failing direct audit test**

Add a test that builds a cell with one `CellDependency`, calls `audit(&cell)`, and asserts the trace includes dependency target, kind, and rationale.

- [x] **Step 2: Write failing checkout slice test**

Add a test that appends a dependent cell to `MemoryKernel`, runs `checkout`, and asserts `slice.audit_traces[0].dependencies` preserves the dependency metadata.

- [x] **Step 3: Verify RED**

Run:

```bash
cargo test -p continuitydb-checkout audit_includes_dependency_links
cargo test -p continuitydb-checkout checkout_audit_traces_include_dependency_links
```

Expected: FAIL because `AuditTrace` has no `dependencies` field and `AuditDependency` does not exist.

- [x] **Step 4: Implement audit dependency metadata**

Add:

```rust
pub struct AuditDependency {
    pub target: StateCellId,
    pub kind: CellDependencyKind,
    pub rationale: String,
}
```

Add `dependencies: Vec<AuditDependency>` to `AuditTrace` and populate it in `audit`.

- [x] **Step 5: Update existing audit expectations**

Update existing tests and CLI JSON assertions to account for the new `dependencies` field.

- [x] **Step 6: Run focused tests**

Run:

```bash
cargo test -p continuitydb-checkout audit
cargo test -p continuitydb-cli
```

Expected: PASS.

- [x] **Step 7: Update roadmap**

Add a checkout/audit milestone noting audit traces now include dependency and causality links.

- [x] **Step 8: Run workspace verification**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

Expected: all commands pass.

- [x] **Step 9: Commit**

```bash
git add crates/continuitydb-checkout/src/lib.rs crates/continuitydb-cli/tests/cli.rs docs/roadmap.md docs/superpowers/plans/2026-05-20-audit-dependencies.md
git commit -m "feat: audit state cell dependencies"
```
