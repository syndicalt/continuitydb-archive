# File Open Requirements Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add file-backed `ContinuityDb` constructors that enforce storage requirements during open.

**Architecture:** Specialized `ContinuityDb<FileKernel>` constructors wrap `FileKernel::open`, then reuse `ensure_kernel_requirements` before returning the database. The generic `ContinuityDb<K>` API remains unchanged and no runtime kernel factory is introduced.

**Tech Stack:** Rust, existing `continuitydb-api` and `continuitydb-kernel` crates, Cargo unit tests, rustfmt, clippy.

---

## File Structure

- Modify `crates/continuitydb-api/src/lib.rs`: add tests and `ContinuityDb<FileKernel>` constructors.
- Modify `README.md`: add file-backed open requirement gate to current scope.
- Modify `docs/roadmap.md`: add native API milestone.

## Task 1: File-Backed Open Constructors

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`

- [ ] **Step 1: Write failing API tests**

Add these tests near the existing kernel requirement tests in `crates/continuitydb-api/src/lib.rs`:

```rust
#[test]
fn api_opens_file_backed_database() -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_file_kernel_path("api-open-file");

    let db = ContinuityDb::open_file(&path)?;

    assert_eq!(db.kernel().path(), path.as_path());
    assert!(db.kernel_satisfies(KernelRequirements::durable_append_log()));

    fs::remove_file(path)?;
    Ok(())
}

#[test]
fn api_open_file_accepts_satisfied_kernel_requirements(
) -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_file_kernel_path("api-open-file-require-durable");

    let db = ContinuityDb::open_file_with_requirements(
        &path,
        KernelRequirements::durable_append_log(),
    )?;

    assert_eq!(db.kernel().path(), path.as_path());

    fs::remove_file(path)?;
    Ok(())
}

#[test]
fn api_open_file_rejects_unsatisfied_kernel_requirements() {
    let path = temp_file_kernel_path("api-open-file-require-indexed");
    let required = KernelRequirements::indexed_embedded();

    let result = ContinuityDb::open_file_with_requirements(&path, required);

    assert!(matches!(
        result,
        Err(ContinuityError::KernelRequirementsNotMet {
            required: error_required,
            actual,
        }) if error_required == required
            && actual.durability == continuitydb_kernel::KernelDurability::AppendLog
            && !actual.persistent_indexes
    ));

    let _ = fs::remove_file(path);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p continuitydb-api open_file --all-features
```

Expected: FAIL because `ContinuityDb::open_file` and `open_file_with_requirements` do not exist.

- [ ] **Step 3: Implement constructors**

Add to the existing `impl ContinuityDb<FileKernel>` block before `compact_file_store`:

```rust
/// Opens a file-backed ContinuityDB instance.
pub fn open_file<P: AsRef<Path>>(path: P) -> Result<Self, ContinuityError> {
    FileKernel::open(path).map(Self::new).map_err(Into::into)
}

/// Opens a file-backed ContinuityDB instance only when the kernel satisfies the requested guarantees.
pub fn open_file_with_requirements<P: AsRef<Path>>(
    path: P,
    requirements: KernelRequirements,
) -> Result<Self, ContinuityError> {
    let db = Self::open_file(path)?;
    db.ensure_kernel_requirements(requirements)?;
    Ok(db)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run:

```bash
cargo test -p continuitydb-api open_file --all-features
```

Expected: PASS.

- [ ] **Step 5: Commit implementation**

```bash
git add crates/continuitydb-api/src/lib.rs
git commit -m "feat: add file open requirement gate"
```

## Task 2: Docs and Full Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [ ] **Step 1: Update docs**

Add this README current-scope bullet near the native file-backed API bullets:

```markdown
- Native file-backed open helpers with requirement enforcement.
```

Add this native API roadmap milestone after kernel requirement enforcement:

```markdown
13. Add file-backed open helpers with requirement enforcement. Implemented `ContinuityDb<FileKernel>::open_file` and `open_file_with_requirements` so embedders can enforce storage profiles before receiving a usable file-backed database handle.
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

- [ ] **Step 3: Commit docs**

```bash
git add README.md docs/roadmap.md
git commit -m "docs: record file open requirement gate"
```
