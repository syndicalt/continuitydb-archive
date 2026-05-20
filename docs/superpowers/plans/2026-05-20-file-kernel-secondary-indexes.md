# File Kernel Secondary Indexes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add deterministic in-process answerability-question and evidence-source indexes to the durable file kernel.

**Architecture:** Extend `FileKernelIndex` with derived `HashMap<String, Vec<usize>>` indexes for answerability questions and evidence sources. Rebuild them from the JSONL log through the existing `insert` path, update them after durable appends, and use them for candidate selection before the existing predicate filters.

**Tech Stack:** Rust, existing `continuitydb-kernel`, in-module tests, no new dependencies.

---

## File Structure

- Modify `crates/continuitydb-kernel/src/lib.rs`: add index fields, update insert and lookup candidate selection, add tests.
- Modify `README.md`: add secondary file-kernel indexes to current scope.
- Modify `docs/roadmap.md`: add Storage Kernel milestone.
- Modify `docs/superpowers/plans/2026-05-20-file-kernel-secondary-indexes.md`: track completed steps.

## Task 1: Failing Secondary Index Tests

**Files:**
- Modify: `crates/continuitydb-kernel/src/lib.rs`

- [ ] **Step 1: Add answerability rebuild test**

Add near `file_kernel_filters_by_answerability_question`:

```rust
#[test]
fn file_kernel_rebuilds_answerability_question_index() -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_kernel_path("continuitydb-file-kernel-answerability-index-rebuild");
    let mut status = sample_cell("project:continuitydb:index-status", 0.91, 12)?;
    status.answerability = Answerability::new(vec!["what is status?".to_string()])?;
    let mut frontier = sample_cell("project:continuitydb:index-frontier", 0.83, 15)?;
    frontier.answerability = Answerability::new(vec!["what is frontier?".to_string()])?;
    {
        let mut kernel = FileKernel::open(&path)?;
        append_committed(&mut kernel, status)?;
        frontier = append_committed(&mut kernel, frontier)?;
    }

    let reopened = FileKernel::open(&path)?;
    let positions = reopened
        .index
        .answerability_questions
        .get("what is frontier?")
        .cloned()
        .unwrap_or_default();
    let indexed = positions
        .iter()
        .map(|position| reopened.index.cells[*position].clone())
        .collect::<Vec<_>>();

    assert_eq!(indexed, vec![frontier]);
    fs::remove_file(path)?;
    Ok(())
}
```

- [ ] **Step 2: Add answerability append-update test**

Add:

```rust
#[test]
fn file_kernel_updates_answerability_question_index_after_append(
) -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_kernel_path("continuitydb-file-kernel-answerability-index-append");
    let mut frontier = sample_cell("project:continuitydb:index-append-frontier", 0.83, 15)?;
    frontier.answerability = Answerability::new(vec!["what changed?".to_string()])?;
    let mut kernel = FileKernel::open(&path)?;

    frontier = append_committed(&mut kernel, frontier)?;
    let positions = kernel
        .index
        .answerability_questions
        .get("what changed?")
        .cloned()
        .unwrap_or_default();
    let indexed = positions
        .iter()
        .map(|position| kernel.index.cells[*position].clone())
        .collect::<Vec<_>>();

    assert_eq!(indexed, vec![frontier]);
    fs::remove_file(path)?;
    Ok(())
}
```

- [ ] **Step 3: Add evidence-source rebuild test**

Add near `file_kernel_filters_by_evidence_source`:

```rust
#[test]
fn file_kernel_rebuilds_evidence_source_index() -> Result<(), Box<dyn std::error::Error>> {
    let path = temp_kernel_path("continuitydb-file-kernel-evidence-index-rebuild");
    let observed = sample_cell_with_source("project:continuitydb:index-observed", "sensor", 0.91, 12)?;
    let mut reviewed =
        sample_cell_with_source("project:continuitydb:index-reviewed", "human", 0.83, 15)?;
    {
        let mut kernel = FileKernel::open(&path)?;
        append_committed(&mut kernel, observed)?;
        reviewed = append_committed(&mut kernel, reviewed)?;
    }

    let reopened = FileKernel::open(&path)?;
    let positions = reopened
        .index
        .evidence_sources
        .get("human")
        .cloned()
        .unwrap_or_default();
    let indexed = positions
        .iter()
        .map(|position| reopened.index.cells[*position].clone())
        .collect::<Vec<_>>();

    assert_eq!(indexed, vec![reviewed]);
    fs::remove_file(path)?;
    Ok(())
}
```

- [ ] **Step 4: Add evidence-source append-update test**

Add:

```rust
#[test]
fn file_kernel_updates_evidence_source_index_after_append() -> Result<(), Box<dyn std::error::Error>>
{
    let path = temp_kernel_path("continuitydb-file-kernel-evidence-index-append");
    let mut reviewed =
        sample_cell_with_source("project:continuitydb:index-append-reviewed", "human", 0.83, 15)?;
    let mut kernel = FileKernel::open(&path)?;

    reviewed = append_committed(&mut kernel, reviewed)?;
    let positions = kernel
        .index
        .evidence_sources
        .get("human")
        .cloned()
        .unwrap_or_default();
    let indexed = positions
        .iter()
        .map(|position| kernel.index.cells[*position].clone())
        .collect::<Vec<_>>();

    assert_eq!(indexed, vec![reviewed]);
    fs::remove_file(path)?;
    Ok(())
}
```

- [ ] **Step 5: Run targeted tests and verify RED**

Run:

```bash
cargo test -p continuitydb-kernel file_kernel_rebuilds_ file_kernel_updates_
```

Expected: compilation fails because `answerability_questions` and `evidence_sources` are not fields on `FileKernelIndex`.

## Task 2: Secondary Index Implementation

**Files:**
- Modify: `crates/continuitydb-kernel/src/lib.rs`

- [ ] **Step 1: Add index fields**

Change `FileKernelIndex` to include:

```rust
answerability_questions: HashMap<String, Vec<usize>>,
evidence_sources: HashMap<String, Vec<usize>>,
```

- [ ] **Step 2: Populate indexes during insert**

In `FileKernelIndex::insert`, after anchor indexing and before commit indexing, add:

```rust
for question in cell.answerability.questions() {
    self.answerability_questions
        .entry(question.clone())
        .or_default()
        .push(position);
}
for evidence in &cell.evidence {
    self.evidence_sources
        .entry(evidence.source.as_str().to_string())
        .or_default()
        .push(position);
}
```

- [ ] **Step 3: Use indexes for candidate selection**

In `FileKernel::lookup_cells`, add answerability/evidence branches after commit lookup and before full scan:

```rust
} else if let Some(question) = lookup.answerability_question.as_ref() {
    self.index
        .answerability_questions
        .get(question)
        .map(|positions| {
            positions
                .iter()
                .map(|position| &self.index.cells[*position])
                .collect()
        })
        .unwrap_or_default()
} else if let Some(source) = lookup.evidence_source.as_ref() {
    self.index
        .evidence_sources
        .get(source)
        .map(|positions| {
            positions
                .iter()
                .map(|position| &self.index.cells[*position])
                .collect()
        })
        .unwrap_or_default()
```

- [ ] **Step 4: Run targeted tests and verify GREEN**

Run:

```bash
cargo test -p continuitydb-kernel file_kernel_rebuilds_
cargo test -p continuitydb-kernel file_kernel_updates_
cargo test -p continuitydb-kernel file_kernel_filters_by_answerability_question
cargo test -p continuitydb-kernel file_kernel_filters_by_evidence_source
```

Expected: all targeted file-kernel index and lookup tests pass.

## Task 3: Docs and Full Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-file-kernel-secondary-indexes.md`

- [ ] **Step 1: Update README**

Add to Current Scope near the existing file-kernel index bullet:

```markdown
- File-kernel secondary indexes for answerability questions and evidence sources.
```

- [ ] **Step 2: Update roadmap**

Add a Storage Kernel milestone after milestone 20:

```markdown
21. Add file-kernel secondary indexes for answerability and evidence source lookups. Implemented derived in-process indexes rebuilt from the JSONL log and maintained after append so common context retrieval filters can start from indexed candidates while preserving append-order results and the canonical log as source of truth.
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
git add README.md docs/roadmap.md crates/continuitydb-kernel/src/lib.rs docs/superpowers/plans/2026-05-20-file-kernel-secondary-indexes.md
git commit -m "feat: add file kernel secondary indexes"
```
