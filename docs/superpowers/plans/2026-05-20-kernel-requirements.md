# Kernel Requirements Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add typed kernel requirement checks so embedders can enforce durable append-log or future indexed embedded storage guarantees through the native API.

**Architecture:** `continuitydb-kernel` defines `KernelRequirements` and `KernelCapabilities::satisfies`. `continuitydb-api` exposes `kernel_satisfies` and `ensure_kernel_requirements`, returning a typed `ContinuityError::KernelRequirementsNotMet` mismatch when requirements are not met.

**Tech Stack:** Rust workspace, existing kernel/API crates, Cargo unit tests, rustfmt, clippy.

---

## File Structure

- Modify `crates/continuitydb-kernel/src/lib.rs`: add `KernelRequirements`, durability comparison, satisfaction tests.
- Modify `crates/continuitydb-api/src/lib.rs`: add API requirement checks, typed error, API tests.
- Modify `README.md`: add kernel requirement enforcement to current scope.
- Modify `docs/roadmap.md`: add storage/native API milestones for requirement checks.

## Task 1: Kernel Requirement Matching

**Files:**
- Modify: `crates/continuitydb-kernel/src/lib.rs`

- [ ] **Step 1: Write failing kernel tests**

Add these tests near the existing capability tests in `crates/continuitydb-kernel/src/lib.rs`:

```rust
#[test]
fn ephemeral_capabilities_satisfy_ephemeral_requirements() {
    assert!(KernelCapabilities::ephemeral().satisfies(KernelRequirements::ephemeral()));
}

#[test]
fn ephemeral_capabilities_do_not_satisfy_durable_append_log_requirements() {
    assert!(!KernelCapabilities::ephemeral().satisfies(KernelRequirements::durable_append_log()));
}

#[test]
fn file_append_log_capabilities_satisfy_durable_append_log_requirements() {
    assert!(
        KernelCapabilities::file_append_log().satisfies(KernelRequirements::durable_append_log())
    );
}

#[test]
fn file_append_log_capabilities_do_not_satisfy_indexed_embedded_requirements() {
    assert!(
        !KernelCapabilities::file_append_log().satisfies(KernelRequirements::indexed_embedded())
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p continuitydb-kernel capabilities --all-features
```

Expected: FAIL because `KernelRequirements` and `KernelCapabilities::satisfies` do not exist.

- [ ] **Step 3: Implement minimal kernel requirement types**

Add after `KernelCapabilities`:

```rust
/// Required storage guarantees for an embedder or operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KernelRequirements {
    /// Minimum acceptable persistence level.
    pub minimum_durability: KernelDurability,
    /// Requires immutable append-only writes.
    pub append_only: bool,
    /// Requires derived in-process indexes.
    pub derived_indexes: bool,
    /// Requires durable persistent indexes.
    pub persistent_indexes: bool,
    /// Requires explicit commit manifest records.
    pub explicit_commit_records: bool,
    /// Requires writes to flush through the filesystem boundary.
    pub durable_flush: bool,
    /// Requires storage compaction support.
    pub compaction: bool,
}
```

Add to `impl KernelDurability`:

```rust
const fn rank(self) -> u8 {
    match self {
        Self::Ephemeral => 0,
        Self::AppendLog => 1,
        Self::IndexedEmbedded => 2,
    }
}

const fn satisfies(self, minimum: Self) -> bool {
    self.rank() >= minimum.rank()
}
```

Add to `impl KernelRequirements`:

```rust
/// Requirements for correctness tests and temporary in-process stores.
pub const fn ephemeral() -> Self {
    Self {
        minimum_durability: KernelDurability::Ephemeral,
        append_only: true,
        derived_indexes: false,
        persistent_indexes: false,
        explicit_commit_records: false,
        durable_flush: false,
        compaction: false,
    }
}

/// Requirements for durable local append-log storage.
pub const fn durable_append_log() -> Self {
    Self {
        minimum_durability: KernelDurability::AppendLog,
        append_only: true,
        derived_indexes: false,
        persistent_indexes: false,
        explicit_commit_records: true,
        durable_flush: true,
        compaction: false,
    }
}

/// Requirements for future production indexed embedded storage.
pub const fn indexed_embedded() -> Self {
    Self {
        minimum_durability: KernelDurability::IndexedEmbedded,
        append_only: true,
        derived_indexes: false,
        persistent_indexes: true,
        explicit_commit_records: true,
        durable_flush: true,
        compaction: false,
    }
}
```

Add to `impl KernelCapabilities`:

```rust
/// Returns true when these capabilities meet all required guarantees.
pub const fn satisfies(self, requirements: KernelRequirements) -> bool {
    self.durability.satisfies(requirements.minimum_durability)
        && (!requirements.append_only || self.append_only)
        && (!requirements.derived_indexes || self.derived_indexes)
        && (!requirements.persistent_indexes || self.persistent_indexes)
        && (!requirements.explicit_commit_records || self.explicit_commit_records)
        && (!requirements.durable_flush || self.durable_flush)
        && (!requirements.compaction || self.compaction)
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
git commit -m "feat: add kernel requirement matching"
```

## Task 2: Native API Requirement Enforcement

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`

- [ ] **Step 1: Write failing API tests**

Update the test import in `crates/continuitydb-api/src/lib.rs`:

```rust
use continuitydb_kernel::{
    CellLookup, CommitManifestLookup, FileKernel, KernelDurability, KernelError,
    KernelRequirements, StorageKernel,
};
```

Add these tests near `api_exposes_kernel_capabilities`:

```rust
#[test]
fn api_accepts_satisfied_kernel_requirements() -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_file_kernel_path("api-kernel-requirements");
    let db = ContinuityDb::new(FileKernel::open(&path)?);

    assert!(db.kernel_satisfies(KernelRequirements::durable_append_log()));
    db.ensure_kernel_requirements(KernelRequirements::durable_append_log())?;

    let _ = std::fs::remove_file(path);
    Ok(())
}

#[test]
fn api_rejects_unsatisfied_kernel_requirements() {
    let db = ContinuityDb::new(MemoryKernel::default());
    let required = KernelRequirements::durable_append_log();
    let actual = db.kernel_capabilities();

    let result = db.ensure_kernel_requirements(required);

    assert!(!db.kernel_satisfies(required));
    assert!(matches!(
        result,
        Err(ContinuityError::KernelRequirementsNotMet {
            required: error_required,
            actual: error_actual,
        }) if error_required == required && error_actual == actual
    ));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p continuitydb-api kernel_requirements --all-features
```

Expected: FAIL because `KernelRequirements`, `kernel_satisfies`, `ensure_kernel_requirements`, and `KernelRequirementsNotMet` are not all implemented yet.

- [ ] **Step 3: Implement API enforcement**

Add `KernelRequirements` to the top-level kernel import:

```rust
use continuitydb_kernel::{
    CellLookup, CommitManifestLookup, FileKernel, KernelCapabilities, KernelError,
    KernelRequirements, StorageKernel,
};
```

Add to `ContinuityError`:

```rust
/// Backing kernel does not satisfy required storage guarantees.
#[error("storage kernel requirements are not met")]
KernelRequirementsNotMet {
    /// Required storage guarantees.
    required: KernelRequirements,
    /// Actual backing kernel guarantees.
    actual: KernelCapabilities,
},
```

Add to `impl<K: StorageKernel> ContinuityDb<K>`:

```rust
/// Returns true when the backing kernel satisfies the requested storage guarantees.
pub fn kernel_satisfies(&self, requirements: KernelRequirements) -> bool {
    self.kernel_capabilities().satisfies(requirements)
}

/// Fails when the backing kernel does not satisfy the requested storage guarantees.
pub fn ensure_kernel_requirements(
    &self,
    required: KernelRequirements,
) -> Result<(), ContinuityError> {
    let actual = self.kernel_capabilities();
    if actual.satisfies(required) {
        Ok(())
    } else {
        Err(ContinuityError::KernelRequirementsNotMet { required, actual })
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run:

```bash
cargo test -p continuitydb-api kernel_requirements --all-features
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/continuitydb-api/src/lib.rs
git commit -m "feat: enforce kernel requirements through api"
```

## Task 3: Docs and Full Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [ ] **Step 1: Update docs**

Add this README current-scope bullet near the capability-introspection bullet:

```markdown
- Typed storage-kernel requirement checks for production readiness gates.
```

Add this roadmap storage milestone after the capability milestone:

```markdown
24. Add typed storage-kernel requirement matching. Implemented `KernelRequirements` and `KernelCapabilities::satisfies` so embedders can express ephemeral, durable append-log, and future indexed embedded storage requirements without duplicating capability comparison logic.
```

Add this native API milestone after the backup helper milestone:

```markdown
12. Add native kernel requirement enforcement. Implemented `ContinuityDb::kernel_satisfies` and `ensure_kernel_requirements` with a typed `KernelRequirementsNotMet` error so embedders can fail early when a backing kernel lacks required production guarantees.
```

- [ ] **Step 2: Run full verification gate**

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
git add README.md docs/roadmap.md
git commit -m "docs: record kernel requirement gates"
```
