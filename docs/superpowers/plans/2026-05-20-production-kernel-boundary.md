# Production Kernel Boundary Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add typed storage-kernel capability introspection so embedders can distinguish correctness kernels, append-log kernels, and future indexed embedded kernels without depending on concrete types.

**Architecture:** Define `KernelDurability` and `KernelCapabilities` in `continuitydb-kernel`, add an infallible `StorageKernel::capabilities()` method, override it for durable kernels, and expose it through `ContinuityDb<K>`. The current JSONL `FileKernel` reports append-log durability with derived indexes and durable flush/compaction, while `MemoryKernel` keeps the default ephemeral profile.

**Tech Stack:** Rust workspace, existing `continuitydb-kernel`, `continuitydb-memory`, and `continuitydb-api` crates, Cargo unit tests, rustfmt, clippy.

---

## File Structure

- Modify `crates/continuitydb-kernel/src/lib.rs`: add capability types, trait method, `FileKernel` override, and kernel tests.
- Modify `crates/continuitydb-memory/src/lib.rs`: import capability types in tests and add memory profile coverage.
- Modify `crates/continuitydb-api/src/lib.rs`: expose `ContinuityDb::kernel_capabilities()` and add API test coverage.
- Modify `README.md`: add the capability boundary to current scope.
- Modify `docs/roadmap.md`: add the storage-kernel milestone.

## Task 1: Kernel Capability Types

**Files:**
- Modify: `crates/continuitydb-kernel/src/lib.rs`

- [ ] **Step 1: Write the failing kernel tests**

Add these tests in the existing `#[cfg(test)] mod tests` in `crates/continuitydb-kernel/src/lib.rs`:

```rust
#[test]
fn default_capabilities_are_ephemeral() {
    let capabilities = KernelCapabilities::ephemeral();

    assert_eq!(KernelDurability::Ephemeral, capabilities.durability);
    assert!(capabilities.append_only);
    assert!(!capabilities.derived_indexes);
    assert!(!capabilities.persistent_indexes);
    assert!(!capabilities.explicit_commit_records);
    assert!(!capabilities.durable_flush);
    assert!(!capabilities.compaction);
}

#[test]
fn file_kernel_reports_append_log_capabilities() -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_kernel_path("file-kernel-capabilities");
    let kernel = FileKernel::open(&path)?;

    let capabilities = kernel.capabilities();

    assert_eq!(KernelDurability::AppendLog, capabilities.durability);
    assert!(capabilities.append_only);
    assert!(capabilities.derived_indexes);
    assert!(!capabilities.persistent_indexes);
    assert!(capabilities.explicit_commit_records);
    assert!(capabilities.durable_flush);
    assert!(capabilities.compaction);

    let _ = std::fs::remove_file(path);
    Ok(())
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p continuitydb-kernel capabilities --all-features
```

Expected: FAIL because `KernelCapabilities` and `KernelDurability` are not defined and `StorageKernel::capabilities()` does not exist.

- [ ] **Step 3: Implement minimal kernel capability contract**

Add near the lookup structs in `crates/continuitydb-kernel/src/lib.rs`:

```rust
/// Broad durability class reported by a storage kernel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KernelDurability {
    /// In-process state with no durable persistence guarantee.
    Ephemeral,
    /// Durable append-log storage where indexes may be rebuilt from the log.
    AppendLog,
    /// Durable embedded storage with persistent indexes.
    IndexedEmbedded,
}

/// Observable storage guarantees reported by a storage kernel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KernelCapabilities {
    /// Broad persistence level.
    pub durability: KernelDurability,
    /// Kernel writes immutable append-only records.
    pub append_only: bool,
    /// Kernel maintains derived in-process indexes.
    pub derived_indexes: bool,
    /// Kernel persists indexes durably rather than rebuilding them from the log.
    pub persistent_indexes: bool,
    /// Kernel stores explicit commit manifest records.
    pub explicit_commit_records: bool,
    /// Kernel flushes durable writes through the filesystem boundary.
    pub durable_flush: bool,
    /// Kernel can rewrite storage into a canonical compacted representation.
    pub compaction: bool,
}

impl KernelCapabilities {
    /// Capabilities for in-process correctness kernels.
    pub const fn ephemeral() -> Self {
        Self {
            durability: KernelDurability::Ephemeral,
            append_only: true,
            derived_indexes: false,
            persistent_indexes: false,
            explicit_commit_records: false,
            durable_flush: false,
            compaction: false,
        }
    }

    /// Capabilities for the JSONL append-log file kernel.
    pub const fn file_append_log() -> Self {
        Self {
            durability: KernelDurability::AppendLog,
            append_only: true,
            derived_indexes: true,
            persistent_indexes: false,
            explicit_commit_records: true,
            durable_flush: true,
            compaction: true,
        }
    }
}
```

Add this default method to `StorageKernel`:

```rust
/// Returns the storage guarantees exposed by this kernel.
fn capabilities(&self) -> KernelCapabilities {
    KernelCapabilities::ephemeral()
}
```

Add this override in `impl StorageKernel for FileKernel`:

```rust
fn capabilities(&self) -> KernelCapabilities {
    KernelCapabilities::file_append_log()
}
```

- [ ] **Step 4: Run test to verify it passes**

Run:

```bash
cargo test -p continuitydb-kernel capabilities --all-features
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/continuitydb-kernel/src/lib.rs
git commit -m "feat: add kernel capability contract"
```

## Task 2: Memory and API Capability Exposure

**Files:**
- Modify: `crates/continuitydb-memory/src/lib.rs`
- Modify: `crates/continuitydb-api/src/lib.rs`

- [ ] **Step 1: Write the failing tests**

In `crates/continuitydb-memory/src/lib.rs`, import the types in tests:

```rust
use continuitydb_kernel::{
    CellLookup, CommitManifestLookup, KernelDurability, KernelError, StorageKernel,
};
```

Add:

```rust
#[test]
fn memory_kernel_reports_ephemeral_capabilities() {
    let kernel = MemoryKernel::default();
    let capabilities = kernel.capabilities();

    assert_eq!(KernelDurability::Ephemeral, capabilities.durability);
    assert!(capabilities.append_only);
    assert!(!capabilities.derived_indexes);
    assert!(!capabilities.persistent_indexes);
    assert!(!capabilities.explicit_commit_records);
    assert!(!capabilities.durable_flush);
    assert!(!capabilities.compaction);
}
```

In `crates/continuitydb-api/src/lib.rs`, import `KernelDurability` in tests:

```rust
use continuitydb_kernel::{
    CellLookup, CommitManifestLookup, FileKernel, KernelDurability, KernelError, StorageKernel,
};
```

Add:

```rust
#[test]
fn api_exposes_kernel_capabilities() {
    let db = ContinuityDb::new(MemoryKernel::default());

    let capabilities = db.kernel_capabilities();

    assert_eq!(KernelDurability::Ephemeral, capabilities.durability);
    assert!(capabilities.append_only);
    assert!(!capabilities.durable_flush);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p continuitydb-memory capabilities --all-features
cargo test -p continuitydb-api kernel_capabilities --all-features
```

Expected: memory test passes through the default trait method after Task 1, and API test FAILS because `ContinuityDb::kernel_capabilities()` does not exist.

- [ ] **Step 3: Add native API exposure**

In the `impl<K: StorageKernel> ContinuityDb<K>` block, add:

```rust
/// Returns the storage guarantees exposed by the backing kernel.
pub fn kernel_capabilities(&self) -> continuitydb_kernel::KernelCapabilities {
    self.kernel.capabilities()
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run:

```bash
cargo test -p continuitydb-memory capabilities --all-features
cargo test -p continuitydb-api kernel_capabilities --all-features
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/continuitydb-memory/src/lib.rs crates/continuitydb-api/src/lib.rs
git commit -m "feat: expose kernel capabilities through api"
```

## Task 3: Roadmap and Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [ ] **Step 1: Update docs**

Add this README current-scope bullet near the file-kernel storage bullets:

```markdown
- Typed storage-kernel capability introspection for embedders.
```

Add this roadmap storage milestone after milestone 22:

```markdown
23. Add typed storage-kernel capability introspection. Implemented `KernelDurability`, `KernelCapabilities`, `StorageKernel::capabilities`, and native API exposure so embedders can distinguish ephemeral, append-log, and future indexed embedded kernels without depending on concrete kernel types.
```

- [ ] **Step 2: Run the full verification gate**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

Expected: all commands exit 0.

- [ ] **Step 3: Commit**

```bash
git add README.md docs/roadmap.md crates/continuitydb-kernel/src/lib.rs crates/continuitydb-memory/src/lib.rs crates/continuitydb-api/src/lib.rs
git commit -m "feat: add production kernel capability boundary"
```
