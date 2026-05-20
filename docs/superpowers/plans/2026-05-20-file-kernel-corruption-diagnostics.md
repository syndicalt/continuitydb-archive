# File Kernel Corruption Diagnostics Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add line-addressed JSONL corruption diagnostics to the file-backed storage kernel.

**Architecture:** Extend `KernelError` with a deterministic `StoreCorruptRecord { line }` variant. Keep semantic validation errors as `StoreCorrupt`, and map only per-line decode/header/checksum failures in `read_log_from_path` to the new variant.

**Tech Stack:** Rust, thiserror, serde_json, existing `continuitydb-kernel` tests.

---

## File Structure

- Modify `crates/continuitydb-kernel/src/lib.rs`: add the error variant, line-aware helpers, and tests.
- Modify `docs/roadmap.md`: add a storage-kernel milestone for file-kernel corruption diagnostics.
- Modify `README.md`: add file-kernel corruption diagnostics to current scope.

## Task 1: Failing Diagnostic Tests

**Files:**
- Modify: `crates/continuitydb-kernel/src/lib.rs`

- [ ] **Step 1: Add malformed JSON line test**

Add this test near the existing `file_kernel_rejects_corrupt_jsonl` test:

```rust
#[test]
fn file_kernel_reports_corrupt_jsonl_line_number() -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_kernel_path("continuitydb-file-kernel-corrupt-line");
    let cell = sample_cell("project:continuitydb:corrupt-line", 0.91, 12)?;
    fs::write(&path, format!("{}\n{{not valid json}}\n", serde_json::to_string(&cell)?))?;

    let result = FileKernel::open(&path);

    assert!(matches!(
        result,
        Err(KernelError::StoreCorruptRecord { line: 2 })
    ));
    fs::remove_file(path)?;
    Ok(())
}
```

- [ ] **Step 2: Add unsupported header line test**

Add:

```rust
#[test]
fn file_kernel_reports_unsupported_header_line_number() -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_kernel_path("continuitydb-file-kernel-unsupported-header-line");
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

    assert!(matches!(
        result,
        Err(KernelError::StoreCorruptRecord { line: 1 })
    ));
    fs::remove_file(path)?;
    Ok(())
}
```

- [ ] **Step 3: Add checksum line test**

Add:

```rust
#[test]
fn file_kernel_reports_checksum_failure_line_number() -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_kernel_path("continuitydb-file-kernel-checksum-line");
    {
        let mut kernel = FileKernel::open(&path)?;
        append_committed(
            &mut kernel,
            sample_cell("project:continuitydb:checksum-line", 0.91, 12)?,
        )?;
    }
    let tampered = fs::read_to_string(&path)?.replace(
        "project:continuitydb:checksum-line",
        "project:continuitydb:checksum-line-tampered",
    );
    fs::write(&path, tampered)?;

    let result = FileKernel::open(&path);

    assert!(matches!(
        result,
        Err(KernelError::StoreCorruptRecord { line: 2 })
    ));
    fs::remove_file(path)?;
    Ok(())
}
```

- [ ] **Step 4: Run targeted tests and verify RED**

Run:

```bash
cargo test -p continuitydb-kernel corrupt_line
```

Expected: compilation fails because `KernelError::StoreCorruptRecord` does not exist yet.

## Task 2: Line-Aware Error Implementation

**Files:**
- Modify: `crates/continuitydb-kernel/src/lib.rs`

- [ ] **Step 1: Add error variant**

Add to `KernelError`:

```rust
/// A specific JSONL file-kernel record is corrupt.
#[error("storage kernel record at line {line} is corrupt")]
StoreCorruptRecord {
    /// One-based physical line number in the JSONL log.
    line: usize,
},
```

- [ ] **Step 2: Add helper**

Add near checksum helpers:

```rust
fn corrupt_record(line: usize) -> KernelError {
    KernelError::StoreCorruptRecord { line }
}
```

- [ ] **Step 3: Make `read_log_from_path` line-aware**

Change the loop to:

```rust
for (line_index, line) in reader.lines().enumerate() {
    let line_number = line_index + 1;
    let line = line.map_err(|_error| KernelError::StoreIo)?;
    if line.trim().is_empty() {
        continue;
    }

    match serde_json::from_str::<FileKernelRecord>(&line) {
        Ok(FileKernelRecord::Header { format, version }) => {
            if seen_header || seen_data {
                return Err(corrupt_record(line_number));
            }
            FileKernelHeader { format, version }
                .validate()
                .map_err(|_error| corrupt_record(line_number))?;
            seen_header = true;
        }
        Ok(FileKernelRecord::Cell { cell, checksum }) => {
            seen_data = true;
            validate_file_record_checksum(cell.as_ref(), checksum.as_deref())
                .map_err(|_error| corrupt_record(line_number))?;
            log.cells.push(*cell);
        }
        Ok(FileKernelRecord::Commit { manifest, checksum }) => {
            seen_data = true;
            validate_file_record_checksum(&manifest, checksum.as_deref())
                .map_err(|_error| corrupt_record(line_number))?;
            log.explicit_manifests.push(manifest);
        }
        Err(_record_error) => {
            seen_data = true;
            log.cells.push(
                serde_json::from_str(&line).map_err(|_cell_error| corrupt_record(line_number))?,
            );
        }
    }
}
```

- [ ] **Step 4: Run targeted tests and verify GREEN**

Run:

```bash
cargo test -p continuitydb-kernel corrupt_line
```

Expected: all new diagnostic tests pass.

## Task 3: Docs and Full Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [ ] **Step 1: Update README**

Add to Current Scope:

```markdown
- Line-addressed JSONL file-kernel corruption diagnostics.
```

- [ ] **Step 2: Update roadmap**

Add Storage Kernel milestone:

```markdown
18. Add line-addressed JSONL file-kernel corruption diagnostics. Implemented `KernelError::StoreCorruptRecord { line }` for decode-time header, checksum, and malformed JSONL failures so operators can locate damaged durable records.
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
git add crates/continuitydb-kernel/src/lib.rs README.md docs/roadmap.md docs/superpowers/specs/2026-05-20-file-kernel-corruption-diagnostics-design.md docs/superpowers/plans/2026-05-20-file-kernel-corruption-diagnostics.md
git commit -m "feat: add file kernel corruption diagnostics"
```
