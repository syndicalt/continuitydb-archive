# Native Direct Commit Copy Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an embeddable direct commit-copy API that replays a cursor-selected commit page from one open ContinuityDB into another without a JSON backup file.

**Architecture:** Compose the existing native export and import-summary paths. The method stays deterministic and non-magical: it exports a page from the source, validates/imports it into the target, and returns the same import summary used by backup file imports.

**Tech Stack:** Rust, `continuitydb-api`, generic `StorageKernel`, existing memory/file kernel test helpers.

---

## File Structure

- Modify `crates/continuitydb-api/src/lib.rs`: add `copy_commits_from` and API tests.
- Modify `README.md`: add direct commit copy to current scope.
- Modify `docs/roadmap.md`: add Native API milestone.

## Task 1: Native Direct Commit Copy API

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`

- [ ] **Step 1: Write failing API tests**

Add these tests near existing commit import/export API tests:

```rust
#[test]
fn api_copies_commit_page_from_source_database() -> Result<(), Box<dyn std::error::Error>> {
    let first_time = Utc
        .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let second_time = Utc
        .with_ymd_and_hms(2026, 5, 20, 12, 30, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let first_commit = CommitId::new();
    let second_commit = CommitId::new();
    let mut source = ContinuityDb::new(MemoryKernel::default());
    source.ingest_cells_at_with_commit_id(
        vec![sample_cell("project:continuitydb:copy-first", 0.91, 12)?],
        first_time,
        first_commit,
    )?;
    source.ingest_cells_at_with_commit_id(
        vec![sample_cell("project:continuitydb:copy-second", 0.83, 15)?],
        second_time,
        second_commit,
    )?;
    let mut target = ContinuityDb::new(MemoryKernel::default());

    let summary = target.copy_commits_from(
        &source,
        CommitManifestLookup {
            after: None,
            limit: Some(1),
        },
    )?;

    assert_eq!(
        summary,
        CommitImportSummary {
            imported_commits: 1,
            next_after: Some(first_commit),
        }
    );
    assert_eq!(target.commit_manifests()?.len(), 1);
    assert!(target.commit_manifest(first_commit)?.is_some());
    assert!(target.commit_manifest(second_commit)?.is_none());
    Ok(())
}

#[test]
fn api_copies_next_commit_page_from_source_database() -> Result<(), Box<dyn std::error::Error>> {
    let first_time = Utc
        .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let second_time = Utc
        .with_ymd_and_hms(2026, 5, 20, 12, 30, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let first_commit = CommitId::new();
    let second_commit = CommitId::new();
    let mut source = ContinuityDb::new(MemoryKernel::default());
    source.ingest_cells_at_with_commit_id(
        vec![sample_cell("project:continuitydb:copy-next-first", 0.91, 12)?],
        first_time,
        first_commit,
    )?;
    source.ingest_cells_at_with_commit_id(
        vec![sample_cell("project:continuitydb:copy-next-second", 0.83, 15)?],
        second_time,
        second_commit,
    )?;
    let mut target = ContinuityDb::new(MemoryKernel::default());

    let summary = target.copy_commits_from(
        &source,
        CommitManifestLookup {
            after: Some(first_commit),
            limit: Some(1),
        },
    )?;

    assert_eq!(
        summary,
        CommitImportSummary {
            imported_commits: 1,
            next_after: Some(second_commit),
        }
    );
    assert_eq!(target.commit_manifests()?.len(), 1);
    assert!(target.commit_manifest(first_commit)?.is_none());
    assert!(target.commit_manifest(second_commit)?.is_some());
    Ok(())
}

#[test]
fn api_copy_commits_from_empty_source_returns_empty_summary(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = ContinuityDb::new(MemoryKernel::default());
    let mut target = ContinuityDb::new(MemoryKernel::default());

    let summary = target.copy_commits_from(&source, CommitManifestLookup::default())?;

    assert_eq!(
        summary,
        CommitImportSummary {
            imported_commits: 0,
            next_after: None,
        }
    );
    assert!(target.commit_manifests()?.is_empty());
    Ok(())
}

#[test]
fn api_copy_commits_rejects_duplicate_target_commit_without_mutation(
) -> Result<(), Box<dyn std::error::Error>> {
    let committed_at = Utc
        .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let commit_id = CommitId::new();
    let mut source = ContinuityDb::new(MemoryKernel::default());
    source.ingest_cells_at_with_commit_id(
        vec![sample_cell("project:continuitydb:copy-duplicate-source", 0.91, 12)?],
        committed_at,
        commit_id,
    )?;
    let mut target = ContinuityDb::new(MemoryKernel::default());
    target.ingest_cells_at_with_commit_id(
        vec![sample_cell("project:continuitydb:copy-duplicate-target", 0.83, 15)?],
        committed_at,
        commit_id,
    )?;

    let result = target.copy_commits_from(&source, CommitManifestLookup::default());

    assert!(matches!(
        result,
        Err(ContinuityError::Kernel(KernelError::DuplicateCommit))
    ));
    assert_eq!(target.commit_manifests()?.len(), 1);
    Ok(())
}

#[test]
fn api_copy_commits_reports_unknown_source_cursor() -> Result<(), Box<dyn std::error::Error>> {
    let source = ContinuityDb::new(MemoryKernel::default());
    let mut target = ContinuityDb::new(MemoryKernel::default());

    let result = target.copy_commits_from(
        &source,
        CommitManifestLookup {
            after: Some(CommitId::new()),
            limit: Some(1),
        },
    );

    assert!(matches!(
        result,
        Err(ContinuityError::Kernel(KernelError::CommitNotFound))
    ));
    assert!(target.commit_manifests()?.is_empty());
    Ok(())
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p continuitydb-api copy_commits --all-features
```

Expected: FAIL because `copy_commits_from` does not exist.

- [ ] **Step 3: Implement direct copy API**

Add this method near `export_commits` and `import_commit_batch` in `impl<K: StorageKernel> ContinuityDb<K>`:

```rust
/// Copies a cursor-selected commit page from another open database into this database.
pub fn copy_commits_from<S: StorageKernel>(
    &mut self,
    source: &ContinuityDb<S>,
    lookup: CommitManifestLookup,
) -> Result<CommitImportSummary, ContinuityError> {
    let batch = source.export_commits(lookup)?;
    self.import_commit_batch_with_summary(batch)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run:

```bash
cargo test -p continuitydb-api copy_commits --all-features
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/continuitydb-api/src/lib.rs
git commit -m "feat: add native direct commit copy"
```

## Task 2: Docs and Full Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [ ] **Step 1: Update docs**

Add this README current-scope bullet near commit backup/sync API bullets:

```markdown
- Native direct commit copy between open databases for local sync.
```

Add this Native API roadmap milestone after commit import summaries:

```markdown
20. Add native direct commit copy. Implemented `copy_commits_from` so embedders can replay cursor-selected commit pages between open databases without JSON file envelopes.
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
git commit -m "docs: record native direct commit copy"
```
