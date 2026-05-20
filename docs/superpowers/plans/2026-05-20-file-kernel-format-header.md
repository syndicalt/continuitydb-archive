# File Kernel Format Header Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a versioned JSONL header record to new file-kernel stores while preserving legacy raw StateCell log compatibility.

**Architecture:** Extend the existing internal `FileKernelRecord` envelope with a `Header` variant. `FileKernel::open` writes a header only when the file is empty, and the reader validates header placement/version while continuing to parse legacy logs without a header.

**Tech Stack:** Rust, serde, serde_json, existing `continuitydb-kernel` file storage code.

---

## File Structure

- Modify `crates/continuitydb-kernel/src/lib.rs`: add header constants/type, parser validation, open-time header write, and file-kernel tests.
- Modify `README.md`: add format header support to current scope.
- Modify `docs/roadmap.md`: add storage kernel milestone.

## Task 1: Failing Header Tests

**Files:**
- Modify: `crates/continuitydb-kernel/src/lib.rs`

- [ ] **Step 1: Write failing tests**

Add these tests near the explicit commit-record tests:

```rust
    #[test]
    fn file_kernel_open_writes_header_for_new_empty_file(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-header-new");

        let kernel = FileKernel::open(&path)?;

        assert_eq!(kernel.path(), path.as_path());
        let lines = fs::read_to_string(&path)?
            .lines()
            .map(serde_json::from_str::<serde_json::Value>)
            .collect::<Result<Vec<_>, _>>()?;
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0]["type"], "header");
        assert_eq!(lines[0]["format"], "continuitydb.file_kernel");
        assert_eq!(lines[0]["version"], 1);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_appends_records_after_header() -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-header-before-records");
        let committed_at = test_commit_time()?;
        let commit_id = CommitId::new();
        let cell = sample_cell("project:continuitydb:header-record-order", 0.91, 12)?;
        {
            let mut kernel = FileKernel::open(&path)?;
            kernel.append_cell_at_with_commit_id(cell, committed_at, commit_id)?;
        }

        let lines = fs::read_to_string(&path)?
            .lines()
            .map(serde_json::from_str::<serde_json::Value>)
            .collect::<Result<Vec<_>, _>>()?;
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0]["type"], "header");
        assert_eq!(lines[1]["type"], "cell");
        assert_eq!(lines[2]["type"], "commit");
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rejects_unsupported_header_version(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-unsupported-header");
        fs::write(
            &path,
            "{}\n",
        )?;
        fs::write(
            &path,
            format!(
                "{}\n",
                serde_json::json!({
                    "type": "header",
                    "format": "continuitydb.file_kernel",
                    "version": 999
                })
            ),
        )?;

        let result = FileKernel::open(&path);

        assert!(matches!(result, Err(KernelError::StoreCorrupt)));
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_kernel_rejects_header_after_data_record() -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_kernel_path("continuitydb-file-kernel-late-header");
        let committed_at = test_commit_time()?;
        let commit_id = CommitId::new();
        let mut cell = sample_cell("project:continuitydb:late-header", 0.91, 12)?;
        cell.system_time = continuitydb_core::SystemTimeRange::open_from(committed_at);
        cell.commit_id = commit_id;
        fs::write(
            &path,
            format!(
                "{}\n{}\n",
                serde_json::json!({
                    "type": "cell",
                    "cell": cell
                }),
                serde_json::json!({
                    "type": "header",
                    "format": "continuitydb.file_kernel",
                    "version": 1
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

Expected: the new-header tests fail because new files do not yet receive a header and header records are not yet parsed as supported records.

## Task 2: Header Type and Parser

**Files:**
- Modify: `crates/continuitydb-kernel/src/lib.rs`

- [ ] **Step 1: Add header constants and type**

Add near `FileKernelRecord`:

```rust
const FILE_KERNEL_FORMAT: &str = "continuitydb.file_kernel";
const FILE_KERNEL_FORMAT_VERSION: u32 = 1;

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
struct FileKernelHeader {
    format: String,
    version: u32,
}

impl FileKernelHeader {
    fn current() -> Self {
        Self {
            format: FILE_KERNEL_FORMAT.to_string(),
            version: FILE_KERNEL_FORMAT_VERSION,
        }
    }

    fn validate(&self) -> Result<(), KernelError> {
        if self.format == FILE_KERNEL_FORMAT && self.version == FILE_KERNEL_FORMAT_VERSION {
            Ok(())
        } else {
            Err(KernelError::StoreCorrupt)
        }
    }
}
```

- [ ] **Step 2: Add header record variant**

Change `FileKernelRecord` to:

```rust
enum FileKernelRecord {
    Header {
        format: String,
        version: u32,
    },
    Cell {
        cell: Box<StateCell>,
    },
    Commit {
        manifest: CommitManifest,
    },
}
```

- [ ] **Step 3: Validate header placement while reading**

In `read_log_from_path`, track `seen_header` and `seen_data`. On `Header`, reject duplicate headers, late headers, or unsupported versions. On `Cell`, `Commit`, and legacy raw cells, set `seen_data = true`.

## Task 3: Open-Time Header Write

**Files:**
- Modify: `crates/continuitydb-kernel/src/lib.rs`

- [ ] **Step 1: Write header for empty files**

Add helper:

```rust
fn ensure_file_header(path: &Path) -> Result<(), KernelError> {
    if fs::metadata(path).map_err(|_error| KernelError::StoreIo)?.len() != 0 {
        return Ok(());
    }
    let header = serde_json::to_string(&FileKernelRecord::Header {
        format: FILE_KERNEL_FORMAT.to_string(),
        version: FILE_KERNEL_FORMAT_VERSION,
    })
    .map_err(|_error| KernelError::StoreCorrupt)?;
    let mut file = OpenOptions::new()
        .append(true)
        .open(path)
        .map_err(|_error| KernelError::StoreIo)?;
    writeln!(file, "{header}").map_err(|_error| KernelError::StoreIo)
}
```

Call `ensure_file_header(&path)?` in `FileKernel::open` after creating the file and before `read_log_from_path`.

- [ ] **Step 2: Run targeted tests and verify GREEN**

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

Add this bullet after `Explicit durable commit records in the JSONL file kernel.`:

```markdown
- Versioned JSONL file-kernel format headers.
```

- [ ] **Step 2: Update roadmap storage milestones**

Add this storage milestone after explicit durable commit records:

```markdown
15. Add versioned JSONL file-kernel format headers. Implemented a supported `header` record for new stores while preserving legacy non-header logs and rejecting unsupported or misplaced headers.
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
git commit -m "feat: add file kernel format headers"
```
