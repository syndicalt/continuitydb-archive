# Commit Export JSON Envelope Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add deterministic versioned JSON encode/decode support for commit export batches.

**Architecture:** Add serde derives to the native export structs and wrap batches in a `CommitExportEnvelope` with fixed format/version fields. Decode validates the envelope before exposing the batch.

**Tech Stack:** Rust, serde, serde_json, existing `continuitydb-api` tests.

---

## File Structure

- Modify `crates/continuitydb-api/Cargo.toml`: add `serde` and `serde_json`.
- Modify `crates/continuitydb-api/src/lib.rs`: add envelope type, constants, error variants, encode/decode helpers, and tests.
- Modify `README.md`: add commit export JSON envelope to current scope.
- Modify `docs/roadmap.md`: add Native API milestone.

## Task 1: Failing Envelope Tests

**Files:**
- Modify: `crates/continuitydb-api/src/lib.rs`

- [ ] **Step 1: Import envelope type in tests**

Change:

```rust
use super::{CommitExportBatch, CommitSlice, ContinuityDb, ContinuityError};
```

to:

```rust
use super::{CommitExportBatch, CommitExportEnvelope, CommitSlice, ContinuityDb, ContinuityError};
```

- [ ] **Step 2: Add JSON roundtrip test**

Add near commit export tests:

```rust
#[test]
fn api_encodes_and_decodes_commit_export_json() -> Result<(), Box<dyn std::error::Error>> {
    let committed_at = Utc
        .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let commit_id = CommitId::new();
    let mut db = ContinuityDb::new(MemoryKernel::default());
    db.ingest_cells_at_with_commit_id(
        vec![sample_cell("project:continuitydb:json-export", 0.91, 12)?],
        committed_at,
        commit_id,
    )?;
    let batch = db.export_commits(CommitManifestLookup::default())?;

    let encoded = ContinuityDb::<MemoryKernel>::encode_commit_export_json(batch.clone())?;
    let envelope: serde_json::Value = serde_json::from_slice(&encoded)?;
    let decoded = ContinuityDb::<MemoryKernel>::decode_commit_export_json(&encoded)?;

    assert_eq!(envelope["format"], "continuitydb.commit_export");
    assert_eq!(envelope["version"], 1);
    assert_eq!(decoded, batch);
    Ok(())
}
```

- [ ] **Step 3: Add unsupported version test**

Add:

```rust
#[test]
fn api_rejects_unsupported_commit_export_json_version() {
    let encoded = br#"{"format":"continuitydb.commit_export","version":999,"batch":{"slices":[],"next_after":null}}"#;

    let result = ContinuityDb::<MemoryKernel>::decode_commit_export_json(encoded);

    assert!(matches!(
        result,
        Err(ContinuityError::InvalidCommitExportEnvelope)
    ));
}
```

- [ ] **Step 4: Add decoded import test**

Add:

```rust
#[test]
fn api_imports_decoded_commit_export_json() -> Result<(), Box<dyn std::error::Error>> {
    let committed_at = Utc
        .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
        .single()
        .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
    let commit_id = CommitId::new();
    let mut source = ContinuityDb::new(MemoryKernel::default());
    source.ingest_cells_at_with_commit_id(
        vec![sample_cell("project:continuitydb:json-import", 0.91, 12)?],
        committed_at,
        commit_id,
    )?;
    let batch = source.export_commits(CommitManifestLookup::default())?;
    let encoded = ContinuityDb::<MemoryKernel>::encode_commit_export_json(batch.clone())?;
    let decoded = ContinuityDb::<MemoryKernel>::decode_commit_export_json(&encoded)?;
    let mut target = ContinuityDb::new(MemoryKernel::default());

    let imported = target.import_commit_batch(decoded)?;

    assert_eq!(imported, 1);
    assert_eq!(target.export_commits(CommitManifestLookup::default())?, batch);
    Ok(())
}
```

- [ ] **Step 5: Run targeted tests and verify RED**

Run:

```bash
cargo test -p continuitydb-api commit_export_json
```

Expected: compilation fails because envelope type and helpers do not exist.

## Task 2: Envelope Implementation

**Files:**
- Modify: `crates/continuitydb-api/Cargo.toml`
- Modify: `crates/continuitydb-api/src/lib.rs`

- [ ] **Step 1: Add dependencies**

Add:

```toml
serde.workspace = true
serde_json.workspace = true
```

- [ ] **Step 2: Add serde import and constants**

Add:

```rust
use serde::{Deserialize, Serialize};

pub const COMMIT_EXPORT_FORMAT: &str = "continuitydb.commit_export";
pub const COMMIT_EXPORT_FORMAT_VERSION: u32 = 1;
```

- [ ] **Step 3: Derive serde for export structs and add envelope**

Change export structs to derive `Serialize, Deserialize`, and add:

```rust
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CommitExportEnvelope {
    pub format: String,
    pub version: u32,
    pub batch: CommitExportBatch,
}
```

- [ ] **Step 4: Add envelope validation**

Add:

```rust
impl CommitExportEnvelope {
    pub fn new(batch: CommitExportBatch) -> Self { ... }
    pub fn validate(&self) -> Result<(), ContinuityError> { ... }
}
```

- [ ] **Step 5: Add errors and helpers**

Add error variants for serde decode/encode and invalid envelope. Add static helpers:

```rust
pub fn encode_commit_export_json(batch: CommitExportBatch) -> Result<Vec<u8>, ContinuityError>
pub fn decode_commit_export_json(bytes: &[u8]) -> Result<CommitExportBatch, ContinuityError>
```

- [ ] **Step 6: Run targeted tests and verify GREEN**

Run:

```bash
cargo test -p continuitydb-api commit_export_json
```

Expected: JSON envelope tests pass.

## Task 3: Docs and Full Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-05-20-commit-export-json-envelope.md`

- [ ] **Step 1: Update README**

Add to Current Scope:

```markdown
- Versioned JSON commit export envelope for backup and sync files.
```

- [ ] **Step 2: Update roadmap**

Add Native API milestone:

```markdown
10. Add versioned JSON commit export envelopes. Implemented `CommitExportEnvelope` plus JSON encode/decode helpers so native commit export batches can be written to files or sync channels with explicit format/version validation.
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
git add Cargo.lock crates/continuitydb-api/Cargo.toml crates/continuitydb-api/src/lib.rs README.md docs/roadmap.md docs/superpowers/specs/2026-05-20-commit-export-json-envelope-design.md docs/superpowers/plans/2026-05-20-commit-export-json-envelope.md
git commit -m "feat: add commit export json envelope"
```
