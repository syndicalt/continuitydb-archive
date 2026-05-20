# File Kernel Indexes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move the durable `FileKernel` one step beyond append-and-scan by rebuilding deterministic in-memory indexes from its JSONL log and maintaining them on append.

**Architecture:** Keep the JSONL log as the authoritative durable representation for now. Add an internal `FileKernelIndex` that owns insertion-ordered cells, a primary ID set, and a semantic-anchor-to-cell-position map rebuilt by `FileKernel::open`; append validates against the ID index before writing and updates indexes only after the log write succeeds. Lookup continues to apply all deterministic predicates, but semantic-anchor lookups can start from indexed candidate positions.

**Tech Stack:** Rust 2021, `continuitydb-kernel`, standard-library collections and file I/O, existing `StateCell` JSONL encoding.

---

### Task 1: Rebuild Indexes On Open

**Files:**
- Modify: `crates/continuitydb-kernel/src/lib.rs`
- Modify: `docs/roadmap.md`
- Create: `docs/superpowers/plans/2026-05-20-file-kernel-indexes.md`

- [x] **Step 1: Write failing duplicate-log test**

Add this test to `crates/continuitydb-kernel/src/lib.rs`:

```rust
#[test]
fn file_kernel_open_rejects_duplicate_ids_in_log() -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_kernel_path("continuitydb-file-kernel-duplicate-log");
    let cell = sample_cell("project:continuitydb:duplicate-log", 0.91, 12)?;
    let encoded = serde_json::to_string(&cell)?;
    fs::write(&path, format!("{encoded}\n{encoded}\n"))?;

    let result = FileKernel::open(&path);

    assert!(matches!(result, Err(KernelError::DuplicateCell)));
    fs::remove_file(path)?;
    Ok(())
}
```

- [x] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p continuitydb-kernel file_kernel_open_rejects_duplicate_ids_in_log
```

Expected: FAIL because `FileKernel::open` currently creates the file but does not validate duplicate StateCell IDs while rebuilding state.

- [x] **Step 3: Implement index structure**

Add internal index state near `FileKernel`:

```rust
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct FileKernelIndex {
    cells: Vec<StateCell>,
    ids: HashSet<StateCellId>,
    anchors: HashMap<String, Vec<usize>>,
}
```

Add helper methods:

```rust
impl FileKernelIndex {
    fn rebuild(cells: Vec<StateCell>) -> Result<Self, KernelError> {
        let mut index = Self::default();
        for cell in cells {
            index.insert(cell)?;
        }
        Ok(index)
    }

    fn insert(&mut self, cell: StateCell) -> Result<(), KernelError> {
        if !self.ids.insert(cell.id) {
            return Err(KernelError::DuplicateCell);
        }
        let position = self.cells.len();
        for anchor in &cell.anchors {
            self.anchors
                .entry(anchor.as_str().to_string())
                .or_default()
                .push(position);
        }
        self.cells.push(cell);
        Ok(())
    }

    fn contains_id(&self, id: StateCellId) -> bool {
        self.ids.contains(&id)
    }
}
```

- [x] **Step 4: Wire index into `FileKernel`**

Change `FileKernel` to:

```rust
pub struct FileKernel {
    path: PathBuf,
    index: FileKernelIndex,
}
```

Change `FileKernel::open` so it calls `read_cells_from_path(&path)?` and `FileKernelIndex::rebuild(cells)?`.

Change `append_cell` so it checks `self.index.contains_id(cell.id)`, writes the encoded cell to the JSONL file, then calls `self.index.insert(cell)`.

Change `lookup_cells` so it starts from indexed anchor candidates when `lookup.semantic_anchor` is present, otherwise from `self.index.cells.iter()`, and then applies the existing filters.

- [x] **Step 5: Run focused tests**

Run:

```bash
cargo test -p continuitydb-kernel file_kernel_open_rejects_duplicate_ids_in_log
cargo test -p continuitydb-kernel file_kernel
```

Expected: PASS.

- [x] **Step 6: Update roadmap**

Add a storage-kernel milestone:

```md
7. Add deterministic in-process indexes for the durable file kernel. Implemented ID and semantic-anchor indexes rebuilt from the JSONL log on open and maintained after successful append, giving the first durable kernel a real indexing boundary while preserving the append log as source of truth.
```

- [x] **Step 7: Run workspace verification**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

Expected: all commands pass.

- [x] **Step 8: Commit**

```bash
git add crates/continuitydb-kernel/src/lib.rs docs/roadmap.md docs/superpowers/plans/2026-05-20-file-kernel-indexes.md
git commit -m "feat: index file kernel cells"
```
