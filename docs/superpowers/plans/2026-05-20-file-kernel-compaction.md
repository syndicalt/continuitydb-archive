# File Kernel Compaction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `FileKernel::compact` to rewrite the JSONL log into the current canonical header, checksummed cell, and checksummed commit record format.

**Architecture:** Keep compaction as a `FileKernel` inherent method. Add internal canonical encoding helpers, write compacted output to a same-directory temporary file, parse it before swapping, then rename over the active file and replace the in-memory index.

**Tech Stack:** Rust standard library filesystem APIs, existing `continuitydb-kernel` file records, serde_json.

---

## File Structure

- Modify `crates/continuitydb-kernel/src/lib.rs`: add canonical log encoding, temporary-path helper, `FileKernel::compact`, and tests.
- Modify `README.md`: add JSONL file-kernel compaction to current scope.
- Modify `docs/roadmap.md`: add storage kernel milestone.

## Task 1: Failing Compaction Tests

**Files:**
- Modify: `crates/continuitydb-kernel/src/lib.rs`

- [ ] **Step 1: Write failing tests**

Add these tests near the file-kernel durability tests:

```rust
    #[test]
    fn file_kernel_compacts_legacy_raw_log_to_canonical_records(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-compact-legacy");
        let committed_at = test_commit_time()?;
        let commit_id = CommitId::new();
        let mut first = sample_cell("project:continuitydb:compact-first", 0.91, 12)?;
        let mut second = sample_cell("project:continuitydb:compact-second", 0.83, 15)?;
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

        let mut kernel = FileKernel::open(&path)?;
        kernel.compact()?;

        let records = fs::read_to_string(&path)?
            .lines()
            .map(serde_json::from_str::<serde_json::Value>)
            .collect::<Result<Vec<_>, _>>()?;
        assert_eq!(records.len(), 4);
        assert_eq!(records[0]["type"], "header");
        assert_eq!(records[1]["type"], "cell");
        assert!(records[1]["checksum"].as_str().is_some());
        assert_eq!(records[2]["type"], "cell");
        assert!(records[2]["checksum"].as_str().is_some());
        assert_eq!(records[3]["type"], "commit");
        assert!(records[3]["checksum"].as_str().is_some());
        assert_eq!(
            records[3]["manifest"]["cell_ids"],
            serde_json::to_value(expected_ids)?
        );
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_compaction_preserves_lookup_and_manifest_listing(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-compact-preserve");
        let first_time = test_commit_time()?;
        let second_time = Utc
            .with_ymd_and_hms(2026, 5, 20, 12, 30, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let first_commit = CommitId::new();
        let second_commit = CommitId::new();
        let first = sample_cell("project:continuitydb:compact-list-first", 0.91, 12)?;
        let second = sample_cell("project:continuitydb:compact-list-second", 0.83, 15)?;
        let second_id = second.id;
        {
            let mut kernel = FileKernel::open(&path)?;
            kernel.append_cells_at_with_commit_id(vec![first], first_time, first_commit)?;
            kernel.append_cells_at_with_commit_id(vec![second], second_time, second_commit)?;
            kernel.compact()?;
        }

        let reopened = FileKernel::open(&path)?;
        let manifests = reopened.list_commit_manifests()?;
        let second_lookup = reopened.lookup_cells(CellLookup {
            cell_id: Some(second_id),
            ..CellLookup::default()
        })?;

        assert_eq!(
            manifests
                .iter()
                .map(|manifest| manifest.commit_id)
                .collect::<Vec<_>>(),
            vec![first_commit, second_commit]
        );
        assert_eq!(second_lookup.len(), 1);
        assert_eq!(second_lookup[0].id, second_id);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_compaction_preserves_cursor_manifest_listing(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-compact-cursor");
        let first_time = test_commit_time()?;
        let second_time = Utc
            .with_ymd_and_hms(2026, 5, 20, 12, 30, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let first_commit = CommitId::new();
        let second_commit = CommitId::new();
        {
            let mut kernel = FileKernel::open(&path)?;
            kernel.append_cells_at_with_commit_id(
                vec![sample_cell("project:continuitydb:compact-cursor-first", 0.91, 12)?],
                first_time,
                first_commit,
            )?;
            kernel.append_cells_at_with_commit_id(
                vec![sample_cell("project:continuitydb:compact-cursor-second", 0.83, 15)?],
                second_time,
                second_commit,
            )?;
            kernel.compact()?;
        }

        let reopened = FileKernel::open(&path)?;
        let manifests = reopened.list_commit_manifests_matching(CommitManifestLookup {
            after: Some(first_commit),
            limit: Some(1),
        })?;

        assert_eq!(manifests.len(), 1);
        assert_eq!(manifests[0].commit_id, second_commit);
        fs::remove_file(path)?;
        Ok(())
    }
```

- [ ] **Step 2: Run targeted tests and verify RED**

Run:

```bash
cargo test -p continuitydb-kernel compact
```

Expected: compile failure because `FileKernel::compact` does not exist.

## Task 2: Canonical Encoding Helpers

**Files:**
- Modify: `crates/continuitydb-kernel/src/lib.rs`

- [ ] **Step 1: Add canonical encoding helper**

Add after checksum helpers:

```rust
fn encode_canonical_log<'a>(
    cells: impl IntoIterator<Item = &'a StateCell>,
    manifests: impl IntoIterator<Item = &'a CommitManifest>,
) -> Result<String, KernelError> {
    let header = FileKernelHeader::current();
    let mut encoded = String::new();
    encoded.push_str(
        &serde_json::to_string(&FileKernelRecord::Header {
            format: header.format,
            version: header.version,
        })
        .map_err(|_error| KernelError::StoreCorrupt)?,
    );
    encoded.push('\n');
    for cell in cells {
        encoded.push_str(
            &serde_json::to_string(&FileKernelRecord::Cell {
                cell: Box::new(cell.clone()),
                checksum: Some(file_record_checksum(cell)?),
            })
            .map_err(|_error| KernelError::StoreCorrupt)?,
        );
        encoded.push('\n');
    }
    for manifest in manifests {
        encoded.push_str(
            &serde_json::to_string(&FileKernelRecord::Commit {
                manifest: manifest.clone(),
                checksum: Some(file_record_checksum(manifest)?),
            })
            .map_err(|_error| KernelError::StoreCorrupt)?,
        );
        encoded.push('\n');
    }
    Ok(encoded)
}
```

- [ ] **Step 2: Use canonical helper in append path for records after header**

Do not use the helper directly for append because append must not rewrite the header. Instead keep append's current encoding of cell and commit records unchanged.

## Task 3: Compaction Method

**Files:**
- Modify: `crates/continuitydb-kernel/src/lib.rs`

- [ ] **Step 1: Add temporary path helper**

Add:

```rust
fn compact_temp_path(path: &Path) -> PathBuf {
    let suffix = format!("compact-{}", Utc::now().timestamp_micros());
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "continuitydb.jsonl".to_string());
    path.with_file_name(format!("{file_name}.{suffix}"))
}
```

- [ ] **Step 2: Add `FileKernel::compact`**

Add to `impl FileKernel`:

```rust
    /// Rewrites the backing JSONL log into the current canonical record format.
    pub fn compact(&mut self) -> Result<(), KernelError> {
        let manifests = self.index.list_manifests();
        let encoded = encode_canonical_log(self.index.cells.iter(), manifests.iter())?;
        let temp_path = compact_temp_path(&self.path);
        {
            let mut temp_file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&temp_path)
                .map_err(|_error| KernelError::StoreIo)?;
            temp_file
                .write_all(encoded.as_bytes())
                .map_err(|_error| KernelError::StoreIo)?;
            temp_file.flush().map_err(|_error| KernelError::StoreIo)?;
        }
        let compacted_log = read_log_from_path(&temp_path)?;
        let compacted_index = FileKernelIndex::rebuild(compacted_log)?;
        fs::rename(&temp_path, &self.path).map_err(|_error| KernelError::StoreIo)?;
        self.index = compacted_index;
        Ok(())
    }
```

- [ ] **Step 3: Run targeted tests and verify GREEN**

Run:

```bash
cargo test -p continuitydb-kernel compact
```

Expected: compaction tests pass.

## Task 4: Docs and Full Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [ ] **Step 1: Update README current scope**

Add this bullet after `Per-record JSONL file-kernel checksums for cell and commit records.`:

```markdown
- JSONL file-kernel compaction into the canonical durable record format.
```

- [ ] **Step 2: Update roadmap storage milestones**

Add this storage milestone after record checksums:

```markdown
17. Add JSONL file-kernel compaction. Implemented `FileKernel::compact` to rewrite stored cells and commit manifests into the latest header plus checksummed cell and commit record format while preserving lookup and manifest listing behavior.
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
git add crates/continuitydb-kernel/src/lib.rs README.md docs/roadmap.md
git commit -m "feat: add file kernel compaction"
```
