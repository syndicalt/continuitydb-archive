# Commit Slice Materialization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a native API operation that materializes cursor-selected commit manifests and their ordered StateCells as commit slices.

**Architecture:** Keep this as an API-layer composition over existing kernel primitives. `CommitSlice` lives in `continuitydb-api` because it packages a `CommitManifest` with hydrated `StateCell`s for native callers without changing the storage-kernel trait.

**Tech Stack:** Rust, existing `continuitydb-api`, `continuitydb-core`, `continuitydb-kernel`, and `continuitydb-memory` crates.

---

## File Structure

- Modify `crates/continuitydb-api/src/lib.rs`: add `CommitSlice`, add `ContinuityDb::commit_slices`, and add tests in the existing API test module.
- Modify `README.md`: add commit slice materialization to current scope.
- Modify `docs/roadmap.md`: add the native API milestone.

## Task 1: Commit Slice API Tests

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`

- [ ] **Step 1: Write the failing tests**

Add these tests near the existing `api_returns_commit_cells_in_manifest_order` tests:

```rust
    #[test]
    fn api_returns_commit_slices_after_cursor_with_limit() -> Result<(), Box<dyn std::error::Error>>
    {
        let first_time = Utc
            .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let second_time = Utc
            .with_ymd_and_hms(2026, 5, 20, 12, 30, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let third_time = Utc
            .with_ymd_and_hms(2026, 5, 20, 13, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let first_commit = CommitId::new();
        let second_commit = CommitId::new();
        let third_commit = CommitId::new();
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let first = sample_cell("project:continuitydb:slice-first", 0.91, 12)?;
        let second_a = sample_cell("project:continuitydb:slice-second-a", 0.83, 15)?;
        let second_b = sample_cell("project:continuitydb:slice-second-b", 0.82, 16)?;
        let third = sample_cell("project:continuitydb:slice-third", 0.77, 18)?;
        let expected_second_ids = vec![second_a.id, second_b.id];

        db.ingest_cells_at_with_commit_id(vec![first], first_time, first_commit)?;
        db.ingest_cells_at_with_commit_id(vec![second_a, second_b], second_time, second_commit)?;
        db.ingest_cells_at_with_commit_id(vec![third], third_time, third_commit)?;

        let slices = db.commit_slices(CommitManifestLookup {
            after: Some(first_commit),
            limit: Some(1),
        })?;

        assert_eq!(slices.len(), 1);
        assert_eq!(slices[0].manifest.commit_id, second_commit);
        assert_eq!(slices[0].manifest.committed_at, second_time);
        assert_eq!(slices[0].manifest.cell_ids, expected_second_ids);
        assert_eq!(
            slices[0]
                .cells
                .iter()
                .map(|cell| cell.id)
                .collect::<Vec<_>>(),
            expected_second_ids
        );
        assert!(slices[0]
            .cells
            .iter()
            .all(|cell| cell.commit_id == second_commit));
        Ok(())
    }

    #[test]
    fn api_commit_slices_reports_unknown_cursor() {
        let db = ContinuityDb::new(MemoryKernel::default());

        let result = db.commit_slices(CommitManifestLookup {
            after: Some(CommitId::new()),
            limit: Some(10),
        });

        assert!(matches!(
            result,
            Err(ContinuityError::Kernel(KernelError::CommitNotFound))
        ));
    }

    #[test]
    fn api_commit_slices_returns_empty_for_empty_database(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let db = ContinuityDb::new(MemoryKernel::default());

        let slices = db.commit_slices(CommitManifestLookup::default())?;

        assert!(slices.is_empty());
        Ok(())
    }
```

- [ ] **Step 2: Run the targeted test command and verify RED**

Run:

```bash
cargo test -p continuitydb-api commit_slices
```

Expected: compile failure because `ContinuityDb::commit_slices` does not exist.

## Task 2: Commit Slice API Implementation

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`

- [ ] **Step 1: Add the API type and method**

Add this type near `ContinuityDb<K>`:

```rust
/// Materialized cells for one database commit boundary.
#[derive(Clone, Debug, PartialEq)]
pub struct CommitSlice {
    /// Commit manifest that defines the boundary and cell order.
    pub manifest: CommitManifest,
    /// StateCells written by the commit, in manifest order.
    pub cells: Vec<StateCell>,
}
```

Add this method after `commit_cells`:

```rust
    /// Returns commit manifests and their StateCells matching deterministic listing constraints.
    pub fn commit_slices(
        &self,
        lookup: CommitManifestLookup,
    ) -> Result<Vec<CommitSlice>, ContinuityError> {
        self.commit_manifests_matching(lookup)?
            .into_iter()
            .map(|manifest| {
                let cells = self.lookup_cells_in_order(manifest.cell_ids.clone())?;
                Ok(CommitSlice { manifest, cells })
            })
            .collect()
    }
```

- [ ] **Step 2: Run the targeted test command and verify GREEN**

Run:

```bash
cargo test -p continuitydb-api commit_slices
```

Expected: the three `commit_slices` tests pass.

## Task 3: Roadmap and README Updates

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [ ] **Step 1: Update README current scope**

Add this bullet after `Native commit cell materialization API.`:

```markdown
- Native commit slice materialization API for cursor-selected commit replay.
```

- [ ] **Step 2: Update roadmap Native API milestones**

Add this milestone after ordered commit cell materialization:

```markdown
6. Add cursor-based commit slice materialization. Implemented `CommitSlice` and `ContinuityDb::commit_slices` so callers can materialize cursor-selected commit manifests with their ordered StateCells for audit, backup, sync, and replay flows.
```

## Task 4: Full Verification and Commit

**Files:**
- Verify all modified files.

- [ ] **Step 1: Run formatting check**

Run:

```bash
cargo fmt --all -- --check
```

Expected: exit 0.

- [ ] **Step 2: Run clippy**

Run:

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Expected: exit 0.

- [ ] **Step 3: Run all feature tests**

Run:

```bash
cargo test --workspace --all-features
```

Expected: exit 0.

- [ ] **Step 4: Run default tests**

Run:

```bash
cargo test --workspace
```

Expected: exit 0.

- [ ] **Step 5: Check whitespace**

Run:

```bash
git diff --check
```

Expected: exit 0.

- [ ] **Step 6: Commit**

Run:

```bash
git add crates/continuitydb-api/src/lib.rs README.md docs/roadmap.md
git commit -m "feat: add commit slice materialization"
```
