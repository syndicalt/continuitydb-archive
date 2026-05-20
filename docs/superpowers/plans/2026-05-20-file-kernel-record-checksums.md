# File Kernel Record Checksums Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add deterministic per-record checksums for new file-kernel cell and commit records while preserving legacy log compatibility.

**Architecture:** Keep checksums internal to `continuitydb-kernel`. Extend `FileKernelRecord::Cell` and `FileKernelRecord::Commit` with optional checksum fields, compute checksums over canonical JSON payload bytes, validate checksummed records on read, and keep checksum-free envelope/raw records readable.

**Tech Stack:** Rust, serde_json, existing `continuitydb-kernel` file storage code, internal FNV-1a 64-bit helper.

---

## File Structure

- Modify `crates/continuitydb-kernel/src/lib.rs`: add checksum helpers, extend internal record variants, update read/write paths, and add tests.
- Modify `README.md`: add file-kernel record checksums to current scope.
- Modify `docs/roadmap.md`: add storage kernel milestone.

## Task 1: Failing Checksum Tests

**Files:**
- Modify: `crates/continuitydb-kernel/src/lib.rs`

- [ ] **Step 1: Write failing tests**

Add these tests near the header and explicit-record tests:

```rust
    #[test]
    fn file_kernel_writes_checksums_for_cell_and_commit_records(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-checksummed-records");
        let committed_at = test_commit_time()?;
        let commit_id = CommitId::new();
        let cell = sample_cell("project:continuitydb:checksummed", 0.91, 12)?;
        {
            let mut kernel = FileKernel::open(&path)?;
            kernel.append_cell_at_with_commit_id(cell, committed_at, commit_id)?;
        }

        let lines = fs::read_to_string(&path)?
            .lines()
            .map(serde_json::from_str::<serde_json::Value>)
            .collect::<Result<Vec<_>, _>>()?;

        assert_eq!(lines[1]["type"], "cell");
        assert!(lines[1]["checksum"]
            .as_str()
            .ok_or_else(|| std::io::Error::other("missing cell checksum"))?
            .starts_with("continuitydb-fnv1a64:"));
        assert_eq!(lines[2]["type"], "commit");
        assert!(lines[2]["checksum"]
            .as_str()
            .ok_or_else(|| std::io::Error::other("missing commit checksum"))?
            .starts_with("continuitydb-fnv1a64:"));
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rejects_tampered_cell_checksum() -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-tampered-cell-checksum");
        let committed_at = test_commit_time()?;
        let commit_id = CommitId::new();
        let cell = sample_cell("project:continuitydb:tampered-cell", 0.91, 12)?;
        {
            let mut kernel = FileKernel::open(&path)?;
            kernel.append_cell_at_with_commit_id(cell, committed_at, commit_id)?;
        }
        let mut records = fs::read_to_string(&path)?
            .lines()
            .map(serde_json::from_str::<serde_json::Value>)
            .collect::<Result<Vec<_>, _>>()?;
        records[1]["cell"]["payload"] = serde_json::json!({
            "Text": "tampered payload"
        });
        fs::write(
            &path,
            records
                .into_iter()
                .map(|record| record.to_string())
                .collect::<Vec<_>>()
                .join("\n")
                + "\n",
        )?;

        let result = FileKernel::open(&path);

        assert!(matches!(result, Err(KernelError::StoreCorrupt)));
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rejects_tampered_commit_checksum() -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-tampered-commit-checksum");
        let committed_at = test_commit_time()?;
        let commit_id = CommitId::new();
        let cell = sample_cell("project:continuitydb:tampered-commit", 0.91, 12)?;
        {
            let mut kernel = FileKernel::open(&path)?;
            kernel.append_cell_at_with_commit_id(cell, committed_at, commit_id)?;
        }
        let mut records = fs::read_to_string(&path)?
            .lines()
            .map(serde_json::from_str::<serde_json::Value>)
            .collect::<Result<Vec<_>, _>>()?;
        records[2]["manifest"]["cell_ids"] = serde_json::json!([]);
        fs::write(
            &path,
            records
                .into_iter()
                .map(|record| record.to_string())
                .collect::<Vec<_>>()
                .join("\n")
                + "\n",
        )?;

        let result = FileKernel::open(&path);

        assert!(matches!(result, Err(KernelError::StoreCorrupt)));
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_still_opens_checksum_free_envelope_records(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-checksum-free-envelope");
        let committed_at = test_commit_time()?;
        let commit_id = CommitId::new();
        let mut cell = sample_cell("project:continuitydb:checksum-free", 0.91, 12)?;
        cell.system_time = continuitydb_core::SystemTimeRange::open_from(committed_at);
        cell.commit_id = commit_id;
        let manifest =
            continuitydb_core::CommitManifest::new(commit_id, committed_at, vec![cell.id]);
        fs::write(
            &path,
            format!(
                "{}\n{}\n{}\n",
                serde_json::json!({
                    "type": "header",
                    "format": "continuitydb.file_kernel",
                    "version": 1
                }),
                serde_json::json!({
                    "type": "cell",
                    "cell": cell
                }),
                serde_json::json!({
                    "type": "commit",
                    "manifest": manifest
                })
            ),
        )?;

        let kernel = FileKernel::open(&path)?;

        assert_eq!(kernel.lookup_cells(CellLookup::default())?.len(), 1);
        assert!(kernel.lookup_commit_manifest(commit_id)?.is_some());
        fs::remove_file(path)?;
        Ok(())
    }
```

- [ ] **Step 2: Run targeted tests and verify RED**

Run:

```bash
cargo test -p continuitydb-kernel file_kernel
```

Expected: `file_kernel_writes_checksums_for_cell_and_commit_records` fails because records do not yet include checksum fields.

## Task 2: Checksum Helpers

**Files:**
- Modify: `crates/continuitydb-kernel/src/lib.rs`

- [ ] **Step 1: Add checksum constants and helpers**

Add near the file-kernel format constants:

```rust
const FILE_KERNEL_CHECKSUM_ALGORITHM: &str = "continuitydb-fnv1a64";
const FNV1A64_OFFSET: u64 = 0xcbf29ce484222325;
const FNV1A64_PRIME: u64 = 0x100000001b3;

fn file_record_checksum<T: serde::Serialize>(payload: &T) -> Result<String, KernelError> {
    let bytes = serde_json::to_vec(payload).map_err(|_error| KernelError::StoreCorrupt)?;
    let mut hash = FNV1A64_OFFSET;
    for byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV1A64_PRIME);
    }
    Ok(format!("{FILE_KERNEL_CHECKSUM_ALGORITHM}:{hash:016x}"))
}

fn validate_file_record_checksum<T: serde::Serialize>(
    payload: &T,
    checksum: Option<&str>,
) -> Result<(), KernelError> {
    if let Some(checksum) = checksum {
        if file_record_checksum(payload)? != checksum {
            return Err(KernelError::StoreCorrupt);
        }
    }
    Ok(())
}
```

## Task 3: Record Read and Write Integration

**Files:**
- Modify: `crates/continuitydb-kernel/src/lib.rs`

- [ ] **Step 1: Add optional checksum fields to records**

Change `FileKernelRecord` variants:

```rust
    Cell {
        cell: Box<StateCell>,
        checksum: Option<String>,
    },
    Commit {
        manifest: CommitManifest,
        checksum: Option<String>,
    },
```

- [ ] **Step 2: Validate checksums while reading**

Update `read_log_from_path` matches:

```rust
            Ok(FileKernelRecord::Cell { cell, checksum }) => {
                seen_data = true;
                validate_file_record_checksum(cell.as_ref(), checksum.as_deref())?;
                log.cells.push(*cell);
            }
            Ok(FileKernelRecord::Commit { manifest, checksum }) => {
                seen_data = true;
                validate_file_record_checksum(&manifest, checksum.as_deref())?;
                log.explicit_manifests.push(manifest);
            }
```

- [ ] **Step 3: Write checksums for new cell and commit records**

Update append encoding:

```rust
            encoded.push_str(
                &serde_json::to_string(&FileKernelRecord::Cell {
                    checksum: Some(file_record_checksum(cell)?),
                    cell: Box::new(cell.clone()),
                })
                .map_err(|_error| KernelError::StoreCorrupt)?,
            );
```

and:

```rust
        encoded.push_str(
            &serde_json::to_string(&FileKernelRecord::Commit {
                checksum: Some(file_record_checksum(&manifest)?),
                manifest: manifest.clone(),
            })
            .map_err(|_error| KernelError::StoreCorrupt)?,
        );
```

- [ ] **Step 4: Run targeted tests and verify GREEN**

Run:

```bash
cargo test -p continuitydb-kernel file_kernel
```

Expected: all file-kernel tests pass.

## Task 4: Docs and Full Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [ ] **Step 1: Update README current scope**

Add this bullet after `Versioned JSONL file-kernel format headers.`:

```markdown
- Per-record JSONL file-kernel checksums for cell and commit records.
```

- [ ] **Step 2: Update roadmap storage milestones**

Add this storage milestone after versioned headers:

```markdown
16. Add per-record JSONL file-kernel checksums. Implemented deterministic checksum fields for new cell and commit records, validation on reopen, and compatibility with checksum-free legacy envelope records.
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
git commit -m "feat: add file kernel record checksums"
```
