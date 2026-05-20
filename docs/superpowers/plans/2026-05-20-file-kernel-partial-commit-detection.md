# File Kernel Partial Commit Detection Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reject current-format JSONL file-kernel logs whose append was truncated before explicit commit records were written.

**Architecture:** Carry a `has_header` flag in `FileKernelLog`. During index rebuild, require headered logs to include explicit commit manifests for every commit group represented by cell records, while preserving headerless legacy reconstruction.

**Tech Stack:** Rust, serde_json, existing `continuitydb-kernel` tests.

---

## File Structure

- Modify `crates/continuitydb-kernel/src/lib.rs`: add tests, extend `FileKernelLog`, and validate headered commit completeness.
- Modify `README.md`: add partial-commit detection to current scope.
- Modify `docs/roadmap.md`: add a storage-kernel milestone.

## Task 1: Failing Partial-Commit Tests

**Files:**
- Modify: `crates/continuitydb-kernel/src/lib.rs`

- [ ] **Step 1: Add single-cell partial current-format test**

Add this test near the explicit commit record tests:

```rust
#[test]
fn file_kernel_rejects_headered_cell_without_commit_record(
) -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_kernel_path("continuitydb-file-kernel-headered-orphan-cell");
    let committed_at = test_commit_time()?;
    let commit_id = CommitId::new();
    let mut cell = sample_cell("project:continuitydb:orphan-cell", 0.91, 12)?;
    cell.system_time = continuitydb_core::SystemTimeRange::open_from(committed_at);
    cell.commit_id = commit_id;
    fs::write(
        &path,
        format!(
            "{}\n{}\n",
            serde_json::json!({
                "type": "header",
                "format": "continuitydb.file_kernel",
                "version": 1
            }),
            serde_json::json!({
                "type": "cell",
                "cell": cell
            })
        ),
    )?;

    let result = FileKernel::open(&path);

    assert!(matches!(result, Err(KernelError::StoreCorrupt)));
    fs::remove_file(path)?;
    Ok(())
}
```

- [ ] **Step 2: Add multi-cell partial current-format test**

Add:

```rust
#[test]
fn file_kernel_rejects_headered_partial_batch_without_commit_record(
) -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_kernel_path("continuitydb-file-kernel-headered-partial-batch");
    let committed_at = test_commit_time()?;
    let commit_id = CommitId::new();
    let mut first = sample_cell("project:continuitydb:partial-first", 0.91, 12)?;
    first.system_time = continuitydb_core::SystemTimeRange::open_from(committed_at);
    first.commit_id = commit_id;
    fs::write(
        &path,
        format!(
            "{}\n{}\n",
            serde_json::json!({
                "type": "header",
                "format": "continuitydb.file_kernel",
                "version": 1
            }),
            serde_json::json!({
                "type": "cell",
                "cell": first
            })
        ),
    )?;

    let result = FileKernel::open(&path);

    assert!(matches!(result, Err(KernelError::StoreCorrupt)));
    fs::remove_file(path)?;
    Ok(())
}
```

- [ ] **Step 3: Run targeted tests and verify RED**

Run:

```bash
cargo test -p continuitydb-kernel headered_ -- --nocapture
```

Expected: the new tests fail because headered logs currently reconstruct commits from orphan cell records.

## Task 2: Headered Explicit-Manifest Enforcement

**Files:**
- Modify: `crates/continuitydb-kernel/src/lib.rs`

- [ ] **Step 1: Add `has_header` to `FileKernelLog`**

Change:

```rust
struct FileKernelLog {
    cells: Vec<StateCell>,
    explicit_manifests: Vec<CommitManifest>,
}
```

to:

```rust
struct FileKernelLog {
    cells: Vec<StateCell>,
    explicit_manifests: Vec<CommitManifest>,
    has_header: bool,
}
```

- [ ] **Step 2: Set `has_header` while reading**

In the header branch of `read_log_from_path`, after successful validation, add:

```rust
log.has_header = true;
```

- [ ] **Step 3: Enforce explicit manifests for headered logs**

In `FileKernelIndex::rebuild`, after applying all explicit manifests, add:

```rust
if log.has_header {
    let explicit_commit_ids = log
        .explicit_manifests
        .iter()
        .map(|manifest| manifest.commit_id)
        .collect::<HashSet<_>>();
    if index
        .commits
        .keys()
        .any(|commit_id| !explicit_commit_ids.contains(commit_id))
    {
        return Err(KernelError::StoreCorrupt);
    }
}
```

When implementing this with the existing loop, preserve duplicate explicit-commit detection.

- [ ] **Step 4: Run targeted tests and verify GREEN**

Run:

```bash
cargo test -p continuitydb-kernel headered_ -- --nocapture
```

Expected: the new partial-commit tests pass and existing headered compatibility tests still pass.

## Task 3: Docs and Full Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-file-kernel-partial-commit-detection.md`

- [ ] **Step 1: Update README**

Add to Current Scope:

```markdown
- Headered JSONL file-kernel partial-commit detection.
```

- [ ] **Step 2: Update roadmap**

Add Storage Kernel milestone:

```markdown
19. Add headered JSONL file-kernel partial-commit detection. Implemented explicit-manifest enforcement for current-format headered logs so crash-truncated cell records are rejected instead of reconstructed as committed truth, while headerless legacy raw logs remain readable.
```

- [ ] **Step 3: Run full verification gate**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

Expected: every command exits 0.

- [ ] **Step 4: Commit**

Run:

```bash
git add crates/continuitydb-kernel/src/lib.rs README.md docs/roadmap.md docs/superpowers/specs/2026-05-20-file-kernel-partial-commit-detection-design.md docs/superpowers/plans/2026-05-20-file-kernel-partial-commit-detection.md
git commit -m "feat: detect partial file kernel commits"
```
