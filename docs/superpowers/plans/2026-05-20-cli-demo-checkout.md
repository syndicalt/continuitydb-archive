# CLI Demo Checkout Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a deterministic CLI command that demonstrates checkout JSON including citations, uncertainty, frontier recommendations, and alternatives.

**Architecture:** Keep the CLI as a thin consumer of library APIs. Build a small in-memory fixture inside the command, run `continuitydb_checkout::checkout`, and serialize the resulting `CheckoutSlice` as JSON so the public tool exercises the embeddable checkout surface.

**Tech Stack:** Rust 2021, `clap`, `serde_json`, `continuitydb-checkout`, `continuitydb-core`, `continuitydb-memory`.

---

### Task 1: RED CLI Test

**Files:**
- Modify: `crates/continuitydb-cli/tests/cli.rs`
- Modify: `crates/continuitydb-cli/Cargo.toml`

- [x] **Step 1: Add serde_json dev dependency**

Add `serde_json.workspace = true` under CLI dev-dependencies so the integration test can parse command output.

- [x] **Step 2: Write failing CLI JSON test**

Add `cli_demo_checkout_outputs_metadata_json`. It should run:

```bash
continuitydb demo-checkout
```

Then parse stdout as JSON and assert it contains:

- one selected cell
- one audit trace
- one uncertainty entry
- one frontier recommendation
- one token-budget alternative

- [x] **Step 3: Verify RED**

Run:

```bash
cargo test -p continuitydb-cli cli_demo_checkout_outputs_metadata_json
```

Expected: command fails because `demo-checkout` is not implemented.

### Task 2: Implement CLI Demo Checkout

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`
- Modify: `crates/continuitydb-cli/Cargo.toml`

- [x] **Step 1: Add runtime dependencies**

Add `chrono.workspace = true`, `continuitydb-kernel`, and `serde_json.workspace = true` to CLI dependencies.

- [x] **Step 2: Add command variant**

Add:

```rust
DemoCheckout,
```

with help text explaining it prints deterministic checkout JSON.

- [x] **Step 3: Build demo cells**

Create helper functions in `main.rs` that build two project-scoped StateCells:

- selected frontier cell: confidence `0.95`, tokens `10`, activation `Frontier`, citation `demo://frontier`
- omitted active cell: confidence `0.90`, tokens `10`, citation `demo://alternative`

- [x] **Step 4: Run checkout and print JSON**

For `demo-checkout`, append both cells to `MemoryKernel`, run checkout with token budget `10`, and print pretty JSON for the returned `CheckoutSlice`.

- [x] **Step 5: Verify GREEN**

Run:

```bash
cargo test -p continuitydb-cli cli_demo_checkout_outputs_metadata_json
cargo test -p continuitydb-cli
```

Expected: CLI tests pass.

### Task 3: Roadmap and Verification

**Files:**
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-cli-demo-checkout.md`

- [x] **Step 1: Update roadmap**

Add a CLI milestone for deterministic checkout JSON output.

- [x] **Step 2: Mark this plan complete**

Check off completed steps in this plan before commit.

- [x] **Step 3: Verify**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

Expected: all checks pass.
