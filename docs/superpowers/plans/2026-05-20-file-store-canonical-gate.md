# File Store Canonical Gate Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a file-specific production gate that rejects readable but compaction-worthy file stores when callers require canonical durable format.

**Architecture:** Reuse `FileKernelHealth` as the source of truth. `continuitydb-api` adds a typed canonicality error plus file-specific helper methods, and `continuitydb-cli inspect-kernel` gets a `--require-canonical` flag that calls the same native API gate.

**Tech Stack:** Rust, existing `continuitydb-api` and `continuitydb-cli` crates, `thiserror`, `clap`, `assert_cmd`, serde_json.

---

## File Structure

- Modify `crates/continuitydb-api/src/lib.rs`: add `FileStoreCompactionRecommended`, `ensure_file_store_canonical`, `open_canonical_file`, and API tests.
- Modify `crates/continuitydb-cli/src/main.rs`: add `inspect-kernel --require-canonical` and enforce the native API gate.
- Modify `crates/continuitydb-cli/tests/cli.rs`: add CLI success and failure tests.
- Modify `README.md`: add canonical file-store requirement gate to current scope.
- Modify `docs/roadmap.md`: add Native API and CLI milestones.

## Task 1: Native API Canonical Gate

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`

- [ ] **Step 1: Write failing API tests**

Add these tests near the file-store health test:

```rust
#[test]
fn api_accepts_canonical_file_store() -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_file_kernel_path("api-file-store-canonical");
    let db = ContinuityDb::open_file(&path)?;

    db.ensure_file_store_canonical()?;

    fs::remove_file(path)?;
    Ok(())
}

#[test]
fn api_rejects_legacy_file_store_when_canonical_required(
) -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_file_kernel_path("api-file-store-canonical-legacy");
    write_legacy_file_store(&path)?;
    let db = ContinuityDb::open_file(&path)?;

    let result = db.ensure_file_store_canonical();

    assert!(matches!(
        result,
        Err(ContinuityError::FileStoreCompactionRecommended { health })
            if health.legacy_raw_cells == 1 && health.compaction_recommended
    ));

    fs::remove_file(path)?;
    Ok(())
}

#[test]
fn api_open_canonical_file_rejects_legacy_file_store(
) -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_file_kernel_path("api-open-canonical-file-legacy");
    write_legacy_file_store(&path)?;

    let result = ContinuityDb::open_canonical_file(&path);

    assert!(matches!(
        result,
        Err(ContinuityError::FileStoreCompactionRecommended { health })
            if health.legacy_raw_cells == 1 && health.compaction_recommended
    ));

    fs::remove_file(path)?;
    Ok(())
}
```

Add this test helper in the API test module:

```rust
fn write_legacy_file_store(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let committed_at = Utc
        .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let commit_id = CommitId::new();
    let mut cell = sample_cell("project:continuitydb:api-canonical-legacy", 0.91, 12)?;
    cell.system_time = continuitydb_core::SystemTimeRange::open_from(committed_at);
    cell.commit_id = commit_id;
    fs::write(path, format!("{}\n", serde_json::to_string(&cell)?))?;
    Ok(())
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p continuitydb-api canonical_file --all-features
```

Expected: FAIL because `ensure_file_store_canonical`, `open_canonical_file`, and `FileStoreCompactionRecommended` do not exist.

- [ ] **Step 3: Implement API gate**

Add the error variant:

```rust
/// File store is readable but should be compacted before use under the requested policy.
#[error("file store compaction is recommended")]
FileStoreCompactionRecommended {
    /// Health report that explains why the store is not canonical.
    health: FileKernelHealth,
},
```

Add methods to `impl ContinuityDb<FileKernel>`:

```rust
/// Opens a file-backed ContinuityDB instance only when the store is already canonical.
pub fn open_canonical_file<P: AsRef<Path>>(path: P) -> Result<Self, ContinuityError> {
    let db = Self::open_file(path)?;
    db.ensure_file_store_canonical()?;
    Ok(db)
}

/// Ensures the file-backed store does not require compaction.
pub fn ensure_file_store_canonical(&self) -> Result<(), ContinuityError> {
    let health = self.file_store_health();
    if health.compaction_recommended {
        Err(ContinuityError::FileStoreCompactionRecommended { health })
    } else {
        Ok(())
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run:

```bash
cargo test -p continuitydb-api canonical_file --all-features
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/continuitydb-api/src/lib.rs
git commit -m "feat: add canonical file store gate"
```

## Task 2: CLI Canonical Gate

**Files:**
- Modify: `crates/continuitydb-cli/src/main.rs`
- Modify: `crates/continuitydb-cli/tests/cli.rs`

- [ ] **Step 1: Write failing CLI tests**

Add these tests near the inspect-kernel tests:

```rust
#[test]
fn cli_inspect_kernel_accepts_canonical_requirement(
) -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_store_path("continuitydb-cli-inspect-require-canonical");

    let output = Command::cargo_bin("continuitydb")?
        .arg("inspect-kernel")
        .arg(&path)
        .arg("--require-canonical")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output)?;

    assert_eq!(
        json["health"]["compaction_recommended"].as_bool(),
        Some(false)
    );

    fs::remove_file(path)?;
    Ok(())
}

#[test]
fn cli_inspect_kernel_rejects_legacy_when_canonical_required(
) -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_store_path("continuitydb-cli-inspect-require-canonical-legacy");
    write_legacy_store(&path)?;

    Command::cargo_bin("continuitydb")?
        .arg("inspect-kernel")
        .arg(&path)
        .arg("--require-canonical")
        .assert()
        .failure()
        .stderr(contains("file store compaction is recommended"));

    fs::remove_file(path)?;
    Ok(())
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p continuitydb-cli canonical_required --all-features
```

Expected: FAIL because `--require-canonical` is not accepted.

- [ ] **Step 3: Implement CLI flag and enforcement**

Update `InspectKernel`:

```rust
InspectKernel {
    /// Path to the JSONL file-backed store.
    store_path: PathBuf,
    /// Required storage profile.
    #[arg(long = "require")]
    require: Option<RequirementProfile>,
    /// Require the store to already be in canonical durable file format.
    #[arg(long = "require-canonical")]
    require_canonical: bool,
},
```

Update the match arm binding:

```rust
Some(Command::InspectKernel {
    store_path,
    require,
    require_canonical,
}) => {
```

After opening the database and before building output:

```rust
if require_canonical {
    db.ensure_file_store_canonical()?;
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run:

```bash
cargo test -p continuitydb-cli canonical_required --all-features
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/continuitydb-cli/src/main.rs crates/continuitydb-cli/tests/cli.rs
git commit -m "feat: add cli canonical file gate"
```

## Task 3: Docs and Full Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [ ] **Step 1: Update docs**

Add this README current-scope bullet near file-store health:

```markdown
- Canonical file-store requirement gate for production open/inspection paths.
```

Add these roadmap milestones:

Native API:

```markdown
16. Add canonical file-store requirement gate. Implemented `ensure_file_store_canonical` and `open_canonical_file` so embedders can reject readable but compaction-worthy file stores without automatic mutation.
```

CLI:

```markdown
8. Add canonical file-store inspection gate. Implemented `continuitydb inspect-kernel --require-canonical` so CI and operators can fail early when a readable file store needs compaction.
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
git commit -m "docs: record canonical file store gate"
```

