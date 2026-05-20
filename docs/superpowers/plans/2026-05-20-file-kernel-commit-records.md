# File Kernel Commit Records Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Persist explicit commit manifest records in the durable file kernel while keeping legacy raw StateCell logs readable.

**Architecture:** Add an internal JSONL envelope in `continuitydb-kernel` only. New writes emit `cell` records followed by one `commit` record; reads accept both envelope records and legacy raw `StateCell` records, rebuilding the same in-memory indexes behind the unchanged `StorageKernel` trait.

**Tech Stack:** Rust, serde, serde_json, existing `continuitydb-kernel` and `continuitydb-core` types.

---

## File Structure

- Modify `crates/continuitydb-kernel/Cargo.toml`: add `serde.workspace = true`.
- Modify `crates/continuitydb-kernel/src/lib.rs`: add internal file-record types, parse new and legacy records, write envelope records, and add file-kernel tests.
- Modify `README.md`: note explicit durable commit records in current scope.
- Modify `docs/roadmap.md`: add storage kernel milestone.

## Task 1: Failing File-Kernel Format Tests

**Files:**
- Modify: `crates/continuitydb-kernel/src/lib.rs`

- [ ] **Step 1: Write failing tests**

Add these tests near the existing file-kernel commit manifest tests:

```rust
    #[test]
    fn file_kernel_writes_explicit_cell_and_commit_records(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-explicit-commit-record");
        let committed_at = test_commit_time()?;
        let commit_id = CommitId::new();
        let first = sample_cell("project:continuitydb:record-first", 0.91, 12)?;
        let second = sample_cell("project:continuitydb:record-second", 0.83, 15)?;
        let expected_ids = vec![first.id, second.id];

        {
            let mut kernel = FileKernel::open(&path)?;
            kernel.append_cells_at_with_commit_id(vec![first, second], committed_at, commit_id)?;
        }

        let lines = fs::read_to_string(&path)?
            .lines()
            .map(serde_json::from_str::<serde_json::Value>)
            .collect::<Result<Vec<_>, _>>()?;

        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0]["type"], "cell");
        assert_eq!(lines[1]["type"], "cell");
        assert_eq!(lines[2]["type"], "commit");
        assert_eq!(lines[2]["manifest"]["commit_id"], serde_json::to_value(commit_id)?);
        assert_eq!(
            lines[2]["manifest"]["cell_ids"],
            serde_json::to_value(expected_ids)?
        );
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_reopens_explicit_commit_records_as_authoritative_manifest(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-reopen-explicit-commit");
        let committed_at = test_commit_time()?;
        let commit_id = CommitId::new();
        let first = sample_cell("project:continuitydb:explicit-first", 0.91, 12)?;
        let second = sample_cell("project:continuitydb:explicit-second", 0.83, 15)?;
        let expected_ids = vec![first.id, second.id];
        {
            let mut kernel = FileKernel::open(&path)?;
            kernel.append_cells_at_with_commit_id(vec![first, second], committed_at, commit_id)?;
        }

        let reopened = FileKernel::open(&path)?;
        let manifest = reopened
            .lookup_commit_manifest(commit_id)?
            .ok_or_else(|| std::io::Error::other("missing manifest"))?;

        assert_eq!(manifest.commit_id, commit_id);
        assert_eq!(manifest.committed_at, committed_at);
        assert_eq!(manifest.cell_ids, expected_ids);
        assert_eq!(
            reopened.list_commit_manifests()?,
            vec![manifest]
        );
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_still_opens_legacy_raw_cell_logs() -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-legacy-raw-cells");
        let committed_at = test_commit_time()?;
        let commit_id = CommitId::new();
        let mut first = sample_cell("project:continuitydb:legacy-first", 0.91, 12)?;
        let mut second = sample_cell("project:continuitydb:legacy-second", 0.83, 15)?;
        first.system_time = continuitydb_core::SystemTimeRange::open_from(committed_at);
        first.commit_id = commit_id;
        second.system_time = continuitydb_core::SystemTimeRange::open_from(committed_at);
        second.commit_id = commit_id;
        let expected_ids = vec![first.id, second.id];
        fs::write(
            &path,
            format!(
                "{}\n{}\n",
                serde_json::to_string(&first)?,
                serde_json::to_string(&second)?
            ),
        )?;

        let kernel = FileKernel::open(&path)?;
        let manifest = kernel
            .lookup_commit_manifest(commit_id)?
            .ok_or_else(|| std::io::Error::other("missing legacy manifest"))?;

        assert_eq!(manifest.cell_ids, expected_ids);
        assert_eq!(kernel.lookup_cells(CellLookup::default())?.len(), 2);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rejects_commit_record_with_missing_cell(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-missing-commit-cell");
        let committed_at = test_commit_time()?;
        let commit_id = CommitId::new();
        let manifest = continuitydb_core::CommitManifest::new(
            commit_id,
            committed_at,
            vec![StateCellId::new()],
        );
        fs::write(
            &path,
            format!(
                "{}\n",
                serde_json::json!({
                    "type": "commit",
                    "manifest": manifest
                })
            ),
        )?;

        let result = FileKernel::open(&path);

        assert!(matches!(result, Err(KernelError::StoreCorrupt)));
        fs::remove_file(path)?;
        Ok(())
    }
```

- [ ] **Step 2: Run targeted tests and verify RED**

Run:

```bash
cargo test -p continuitydb-kernel file_kernel
```

Expected: at least `file_kernel_writes_explicit_cell_and_commit_records` fails because writes still emit raw `StateCell` lines.

## Task 2: File Record Types and Parser

**Files:**
- Modify: `crates/continuitydb-kernel/Cargo.toml`
- Modify: `crates/continuitydb-kernel/src/lib.rs`

- [ ] **Step 1: Add serde dependency**

Add to `crates/continuitydb-kernel/Cargo.toml`:

```toml
serde.workspace = true
```

- [ ] **Step 2: Add internal record and log types**

Add near `FileKernelIndex`:

```rust
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum FileKernelRecord {
    Cell { cell: StateCell },
    Commit { manifest: CommitManifest },
}

#[derive(Clone, Debug, Default, PartialEq)]
struct FileKernelLog {
    cells: Vec<StateCell>,
    explicit_manifests: Vec<CommitManifest>,
}
```

- [ ] **Step 3: Replace raw cell reader with mixed record reader**

Replace `read_cells_from_path` with:

```rust
fn read_log_from_path(path: &Path) -> Result<FileKernelLog, KernelError> {
    let file = File::open(path).map_err(|_error| KernelError::StoreIo)?;
    let reader = BufReader::new(file);
    let mut log = FileKernelLog::default();

    for line in reader.lines() {
        let line = line.map_err(|_error| KernelError::StoreIo)?;
        if line.trim().is_empty() {
            continue;
        }

        match serde_json::from_str::<FileKernelRecord>(&line) {
            Ok(FileKernelRecord::Cell { cell }) => log.cells.push(cell),
            Ok(FileKernelRecord::Commit { manifest }) => log.explicit_manifests.push(manifest),
            Err(_record_error) => log.cells.push(
                serde_json::from_str(&line).map_err(|_cell_error| KernelError::StoreCorrupt)?,
            ),
        }
    }

    Ok(log)
}
```

Update `FileKernel::open` to call `read_log_from_path`.

## Task 3: Explicit Manifest Indexing

**Files:**
- Modify: `crates/continuitydb-kernel/src/lib.rs`

- [ ] **Step 1: Add explicit rebuild entry point**

Change `FileKernelIndex::rebuild` to accept `FileKernelLog` and apply explicit manifests after inserting cells:

```rust
    fn rebuild(log: FileKernelLog) -> Result<Self, KernelError> {
        let mut index = Self::default();
        for cell in log.cells {
            index.insert(cell)?;
        }
        for manifest in log.explicit_manifests {
            index.apply_explicit_manifest(manifest)?;
        }
        Ok(index)
    }
```

- [ ] **Step 2: Add explicit manifest validator**

Add this method to `FileKernelIndex`:

```rust
    fn apply_explicit_manifest(&mut self, manifest: CommitManifest) -> Result<(), KernelError> {
        let existing_positions = self
            .commits
            .get(&manifest.commit_id)
            .ok_or(KernelError::StoreCorrupt)?;
        let existing_ids = existing_positions
            .iter()
            .map(|position| self.cells[*position].id)
            .collect::<Vec<_>>();
        if existing_ids != manifest.cell_ids {
            return Err(KernelError::StoreCorrupt);
        }
        if manifest
            .cell_ids
            .iter()
            .any(|cell_id| self.cells[self.ids[cell_id]].commit_id != manifest.commit_id)
        {
            return Err(KernelError::StoreCorrupt);
        }

        let manifest_index = self
            .manifest_order
            .iter()
            .position(|commit_id| *commit_id == manifest.commit_id)
            .ok_or(KernelError::StoreCorrupt)?;
        self.manifest_order.remove(manifest_index);
        self.manifest_order.push(manifest.commit_id);
        self.manifests.insert(manifest.commit_id, manifest);
        Ok(())
    }
```

## Task 4: Envelope Writes

**Files:**
- Modify: `crates/continuitydb-kernel/src/lib.rs`

- [ ] **Step 1: Write cell and commit records for new batches**

In `append_cells_at_with_commit_id`, build a manifest before encoding:

```rust
        let manifest = CommitManifest::new(
            commit_id,
            committed_at,
            stamped.iter().map(|cell| cell.id).collect(),
        );
```

Replace raw cell encoding with:

```rust
        let mut encoded = String::new();
        for cell in &stamped {
            encoded.push_str(
                &serde_json::to_string(&FileKernelRecord::Cell { cell: cell.clone() })
                    .map_err(|_error| KernelError::StoreCorrupt)?,
            );
            encoded.push('\n');
        }
        encoded.push_str(
            &serde_json::to_string(&FileKernelRecord::Commit {
                manifest: manifest.clone(),
            })
            .map_err(|_error| KernelError::StoreCorrupt)?,
        );
        encoded.push('\n');
```

After writing, insert cells then apply the explicit manifest:

```rust
        for cell in stamped {
            self.index.insert(cell)?;
        }
        self.index.apply_explicit_manifest(manifest)?;
```

- [ ] **Step 2: Run targeted tests and verify GREEN**

Run:

```bash
cargo test -p continuitydb-kernel file_kernel
```

Expected: explicit record, legacy raw log, corrupt manifest, and existing file-kernel tests pass.

## Task 5: Docs and Full Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [ ] **Step 1: Update README current scope**

Add this bullet after `Cursor-based commit manifest listing for incremental audit and sync reads.`:

```markdown
- Explicit durable commit records in the JSONL file kernel.
```

- [ ] **Step 2: Update roadmap storage milestones**

Add this storage milestone after cursor-based commit manifest listing:

```markdown
14. Add explicit durable commit records to the JSONL file kernel. Implemented backward-compatible `cell` and `commit` file records so new writes persist commit manifests directly while legacy raw StateCell logs remain readable.
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
git add crates/continuitydb-kernel/Cargo.toml crates/continuitydb-kernel/src/lib.rs README.md docs/roadmap.md
git commit -m "feat: add durable commit records"
```
