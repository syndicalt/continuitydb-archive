# API File Compaction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a native API maintenance method that compacts file-backed ContinuityDB stores through `ContinuityDb<FileKernel>`.

**Architecture:** Keep compaction out of the generic `StorageKernel` trait. Add a `FileKernel`-specific inherent impl in `continuitydb-api` that delegates to `FileKernel::compact`, then test through the public API boundary.

**Tech Stack:** Rust, `continuitydb-api`, `continuitydb-kernel::FileKernel`, serde_json for file-shape assertions in tests.

---

## File Structure

- Modify `crates/continuitydb-api/src/lib.rs`: import `FileKernel`, add `compact_file_store`, add tests.
- Modify `README.md`: add native file compaction API to current scope.
- Modify `docs/roadmap.md`: add native API milestone.

## Task 1: Failing API Tests

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`

- [ ] **Step 1: Write failing tests**

Add `FileKernel` to the test imports:

```rust
    use continuitydb_kernel::{
        CellLookup, CommitManifestLookup, FileKernel, KernelError, StorageKernel,
    };
```

Add a helper in the test module:

```rust
    fn temp_file_kernel_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("{name}-{:?}.jsonl", StateCellId::new()))
    }
```

Add these tests near the commit slice tests:

```rust
    #[test]
    fn api_compacts_file_store_to_canonical_records() -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_file_kernel_path("continuitydb-api-file-compact");
        let committed_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let commit_id = CommitId::new();
        let cell = sample_cell("project:continuitydb:api-file-compact", 0.91, 12)?;
        let cell_id = cell.id;
        let mut db = ContinuityDb::new(FileKernel::open(&path)?);
        db.ingest_cell_at_with_commit_id(cell, committed_at, commit_id)?;

        db.compact_file_store()?;

        let records = std::fs::read_to_string(&path)?
            .lines()
            .map(serde_json::from_str::<serde_json::Value>)
            .collect::<Result<Vec<_>, _>>()?;
        assert_eq!(records.len(), 3);
        assert_eq!(records[0]["type"], "header");
        assert_eq!(records[1]["type"], "cell");
        assert!(records[1]["checksum"].as_str().is_some());
        assert_eq!(records[2]["type"], "commit");
        assert!(records[2]["checksum"].as_str().is_some());
        assert_eq!(db.commit_cells(commit_id)?[0].id, cell_id);
        std::fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn api_file_compaction_preserves_commit_slices_after_reopen(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_file_kernel_path("continuitydb-api-file-compact-slices");
        let committed_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let commit_id = CommitId::new();
        let first = sample_cell("project:continuitydb:api-file-compact-first", 0.91, 12)?;
        let second = sample_cell("project:continuitydb:api-file-compact-second", 0.83, 15)?;
        let expected_ids = vec![first.id, second.id];
        {
            let mut db = ContinuityDb::new(FileKernel::open(&path)?);
            db.ingest_cells_at_with_commit_id(vec![first, second], committed_at, commit_id)?;
            db.compact_file_store()?;
        }

        let reopened = ContinuityDb::new(FileKernel::open(&path)?);
        let slices = reopened.commit_slices(CommitManifestLookup::default())?;

        assert_eq!(slices.len(), 1);
        assert_eq!(slices[0].manifest.commit_id, commit_id);
        assert_eq!(
            slices[0]
                .cells
                .iter()
                .map(|cell| cell.id)
                .collect::<Vec<_>>(),
            expected_ids
        );
        std::fs::remove_file(path)?;
        Ok(())
    }
```

- [ ] **Step 2: Run targeted tests and verify RED**

Run:

```bash
cargo test -p continuitydb-api file_compaction
```

Expected: compile failure because `compact_file_store` does not exist.

## Task 2: API Implementation

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`

- [ ] **Step 1: Import `FileKernel`**

Change the kernel import to:

```rust
use continuitydb_kernel::{CellLookup, CommitManifestLookup, FileKernel, KernelError, StorageKernel};
```

- [ ] **Step 2: Add FileKernel-specific API impl**

Add after the generic `impl<K: StorageKernel> ContinuityDb<K>` block:

```rust
impl ContinuityDb<FileKernel> {
    /// Rewrites a file-backed store into the current canonical durable record format.
    pub fn compact_file_store(&mut self) -> Result<(), ContinuityError> {
        self.kernel.compact().map_err(Into::into)
    }
}
```

- [ ] **Step 3: Run targeted tests and verify GREEN**

Run:

```bash
cargo test -p continuitydb-api file_compaction
```

Expected: both file compaction API tests pass.

## Task 3: Docs and Full Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [ ] **Step 1: Update README current scope**

Add this bullet after `JSONL file-kernel compaction into the canonical durable record format.`:

```markdown
- Native file-backed compaction API.
```

- [ ] **Step 2: Update roadmap Native API milestones**

Add this milestone after cursor-based commit slice materialization:

```markdown
7. Add native file-backed compaction API. Implemented `ContinuityDb<FileKernel>::compact_file_store` so embedders can run JSONL store compaction through the native API without expanding the generic storage-kernel trait.
```

- [ ] **Step 3: Run full verification**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

Expected: all commands exit 0.

- [ ] **Step 4: Commit**

Run:

```bash
git add crates/continuitydb-api/src/lib.rs README.md docs/roadmap.md
git commit -m "feat: add api file compaction"
```
