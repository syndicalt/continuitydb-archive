# File Kernel Durable Flush Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add explicit filesystem flush/sync boundaries to file-kernel header writes, append writes, and compaction replacement.

**Architecture:** Introduce small internal durability helpers in `continuitydb-kernel` and route existing file writes through them. Preserve the existing storage trait and JSONL format.

**Tech Stack:** Rust standard library `std::fs::File`, `sync_all`, existing `continuitydb-kernel` tests.

---

## File Structure

- Modify `crates/continuitydb-kernel/src/lib.rs`: add helper tests, helper functions, and use helpers in header/append/compaction writes.
- Modify `README.md`: add durable file flush boundaries to current scope.
- Modify `docs/roadmap.md`: add a storage-kernel milestone.

## Task 1: Failing Durability Helper Tests

**Files:**
- Modify: `crates/continuitydb-kernel/src/lib.rs`

- [x] **Step 1: Add durable write helper test**

Add near the test helper functions:

```rust
#[test]
fn file_kernel_durable_write_helper_persists_bytes() -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_kernel_path("continuitydb-file-kernel-durable-write");
    {
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)?;
        write_all_durable(&mut file, b"{\"type\":\"test\"}\n")?;
    }

    assert_eq!(fs::read_to_string(&path)?, "{\"type\":\"test\"}\n");
    fs::remove_file(path)?;
    Ok(())
}
```

- [x] **Step 2: Add parent-directory sync helper test**

Add:

```rust
#[test]
fn file_kernel_parent_directory_sync_accepts_existing_parent(
) -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_kernel_path("continuitydb-file-kernel-sync-parent");
    fs::write(&path, "")?;

    sync_parent_directory(&path)?;

    fs::remove_file(path)?;
    Ok(())
}
```

- [x] **Step 3: Run targeted tests and verify RED**

Run:

```bash
cargo test -p continuitydb-kernel durable_write sync_parent
```

Expected: command fails because Cargo accepts only one test filter. Then run:

```bash
cargo test -p continuitydb-kernel durable_write
cargo test -p continuitydb-kernel sync_parent
```

Expected: compilation fails because `write_all_durable` and `sync_parent_directory` do not exist yet.

## Task 2: Durable Write Implementation

**Files:**
- Modify: `crates/continuitydb-kernel/src/lib.rs`

- [x] **Step 1: Add durable write helper**

Add near `ensure_file_header`:

```rust
fn write_all_durable(file: &mut File, bytes: &[u8]) -> Result<(), KernelError> {
    file.write_all(bytes)
        .map_err(|_error| KernelError::StoreIo)?;
    file.flush().map_err(|_error| KernelError::StoreIo)?;
    file.sync_all().map_err(|_error| KernelError::StoreIo)
}
```

- [x] **Step 2: Add parent directory sync helper**

Add:

```rust
fn sync_parent_directory(path: &Path) -> Result<(), KernelError> {
    let Some(parent) = path.parent().filter(|parent| !parent.as_os_str().is_empty()) else {
        return Ok(());
    };
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|_error| KernelError::StoreIo)
}
```

- [x] **Step 3: Use durable write for new headers**

In `ensure_file_header`, replace:

```rust
writeln!(file, "{encoded}").map_err(|_error| KernelError::StoreIo)
```

with:

```rust
let record = format!("{encoded}\n");
write_all_durable(&mut file, record.as_bytes())
```

- [x] **Step 4: Use durable write for append batches**

In `append_cells_at_with_commit_id`, replace:

```rust
file.write_all(encoded.as_bytes())
    .map_err(|_error| KernelError::StoreIo)?;
```

with:

```rust
write_all_durable(&mut file, encoded.as_bytes())?;
```

- [x] **Step 5: Use durable write and directory sync for compaction**

In `FileKernel::compact`, replace the temp-file `write_all` and `flush` calls with:

```rust
write_all_durable(&mut temp_file, encoded.as_bytes())?;
```

After `fs::rename(&temp_path, &self.path)?`, add:

```rust
sync_parent_directory(&self.path)?;
```

- [x] **Step 6: Run targeted tests and verify GREEN**

Run:

```bash
cargo test -p continuitydb-kernel durable_write
cargo test -p continuitydb-kernel parent_directory_sync
cargo test -p continuitydb-kernel file_kernel_persists_cells_across_reopen
```

Expected: all targeted tests pass.

## Task 3: Docs and Full Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-file-kernel-durable-flush.md`

- [x] **Step 1: Update README**

Add to Current Scope:

```markdown
- Durable filesystem flush boundaries for JSONL file-kernel writes.
```

- [x] **Step 2: Update roadmap**

Add Storage Kernel milestone:

```markdown
20. Add durable filesystem flush boundaries for JSONL file-kernel writes. Implemented internal durable write helpers using flush plus `sync_all` for headers, append batches, and compaction temp files, with parent-directory sync after compaction rename.
```

- [x] **Step 3: Run full verification gate**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

Expected: every command exits 0.

- [x] **Step 4: Commit**

Run:

```bash
git add crates/continuitydb-kernel/src/lib.rs README.md docs/roadmap.md docs/superpowers/specs/2026-05-20-file-kernel-durable-flush-design.md docs/superpowers/plans/2026-05-20-file-kernel-durable-flush.md
git commit -m "feat: add file kernel durable flushes"
```
