# File Store Revision Link Status Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Report native revision-link record counts from file-store status and CLI inspection.

**Architecture:** Add `revision_link_count` to `FileKernelStatus`, populate it from the validated in-memory file-kernel index, and serialize it through existing native API and CLI status paths. The durable file format and storage kernel trait stay unchanged.

**Tech Stack:** Rust 2021, `continuitydb-kernel`, `continuitydb-api`, `continuitydb-cli`, JSON CLI assertions.

---

### Task 1: RED Status Tests

**Files:**
- Modify: `crates/continuitydb-kernel/src/lib.rs`
- Modify: `crates/continuitydb-api/src/lib.rs`
- Modify: `crates/continuitydb-cli/tests/cli.rs`

- [x] **Step 1: Add failing status assertions**

Add or extend tests so they assert:

- `FileKernel::status().revision_link_count == 0` for empty stores,
- `FileKernel::status().revision_link_count == 1` after appending one native revision-link record,
- `ContinuityDb<FileKernel>::file_store_status().revision_link_count == 1` after recording one native revision link,
- `continuitydb inspect-kernel` JSON includes `status.revision_link_count`.

- [x] **Step 2: Verify RED**

Run:

```bash
cargo test -p continuitydb-kernel file_kernel_status --all-features
cargo test -p continuitydb-api api_reports_file_store_status --all-features
cargo test -p continuitydb-cli cli_inspect_kernel_reports_file_capabilities --all-features
```

Expected: FAIL because `FileKernelStatus` has no revision-link count field and CLI status JSON has no matching property.

### Task 2: GREEN Implementation

**Files:**
- Modify: `crates/continuitydb-kernel/src/lib.rs`
- Modify: `crates/continuitydb-cli/src/main.rs`

- [x] **Step 1: Extend status type**

Add `pub revision_link_count: usize` to `FileKernelStatus`.

- [x] **Step 2: Populate count**

Set `revision_link_count: self.index.revision_links.len()` in `FileKernel::status`.

- [x] **Step 3: Serialize count**

Add `"revision_link_count": status.revision_link_count` to CLI `file_status_json`.

- [x] **Step 4: Verify GREEN**

Run:

```bash
cargo test -p continuitydb-kernel file_kernel_status --all-features
cargo test -p continuitydb-api api_reports_file_store_status --all-features
cargo test -p continuitydb-cli cli_inspect_kernel_reports_file_capabilities --all-features
```

Expected: PASS.

### Task 3: Docs, Full Gate, Commit

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-file-store-revision-link-status.md`

- [x] **Step 1: Update docs**

Record revision-link-aware file-store status in README current scope and roadmap storage/API/CLI status descriptions.

- [x] **Step 2: Run full verification**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

- [x] **Step 3: Commit**

Commit with:

```bash
git add README.md docs/roadmap.md crates/continuitydb-kernel/src/lib.rs crates/continuitydb-api/src/lib.rs crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs docs/superpowers/specs/2026-05-20-file-store-revision-link-status-design.md docs/superpowers/plans/2026-05-20-file-store-revision-link-status.md
git commit -m "feat: report revision link store status"
```
