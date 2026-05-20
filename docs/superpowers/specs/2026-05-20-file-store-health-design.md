# File Store Health Design

## Purpose

Add an operational health report for file-backed stores. The report should tell embedders and CLI users whether an opened store is already in the current canonical durable format, whether it is readable but legacy-compatible, and whether compaction is recommended.

This follows file-store status: status answers "how large is this validated store?", while health answers "what durable format shape did this validated store have when opened?"

## Design Options Considered

1. Re-read the file every time health is requested.
   - Pros: reflects external file edits after open.
   - Cons: duplicates validation work, introduces a second parsing path, and can race with the open handle.

2. Preserve parse metadata from the validated open path.
   - Pros: reuses the single authoritative validation path, is cheap, deterministic, and matches the in-memory view the handle is using.
   - Cons: reports health as of open/last compaction, not arbitrary external mutation.

3. Make health part of the generic `StorageKernel` trait.
   - Pros: one generic API surface.
   - Cons: premature. Memory kernels and future indexed kernels will have different health dimensions.

Recommended approach: option 2. `FileKernel` should keep file-format metadata produced by `read_log_from_path`, update it after compaction, and expose a file-specific report. The generic kernel trait stays unchanged.

## Data Model

Add a public `FileKernelHealth` struct in `continuitydb-kernel`:

```rust
pub struct FileKernelHealth {
    pub has_header: bool,
    pub legacy_raw_cells: usize,
    pub checksum_free_records: usize,
    pub canonical_records: usize,
    pub compaction_recommended: bool,
}
```

Field semantics:

- `has_header`: true when the opened log had the supported current file-kernel header.
- `legacy_raw_cells`: number of legacy raw `StateCell` JSONL records accepted during open.
- `checksum_free_records`: number of typed `cell` or `commit` envelope records that lacked checksums.
- `canonical_records`: number of typed `cell` or `commit` records with valid checksums.
- `compaction_recommended`: true when the store lacks the current header, has legacy raw cells, or has checksum-free typed records.

Header records are not counted as canonical records. Empty new stores have a header and zero data records, so they are healthy and do not need compaction.

## Storage Mechanics

Extend the internal `FileKernelLog` with a `health: FileKernelHealth` field. `read_log_from_path` should increment health counters while parsing:

- supported header sets `has_header = true`;
- typed cell/commit with `Some(checksum)` increments `canonical_records`;
- typed cell/commit with `None` increments `checksum_free_records`;
- fallback raw `StateCell` increments `legacy_raw_cells`.

`FileKernel::open` stores this health next to the index. `FileKernel::compact` already rewrites the log into the current canonical format and reparses the temp file before renaming; it should replace both `index` and `health` from the compacted parse result.

Append behavior should preserve the report's open-time legacy facts while incrementing `canonical_records` for new durable `cell` and `commit` records written by the append. That makes health stable enough for operators: a legacy store continues to recommend compaction until compaction actually rewrites it.

## Native API and CLI

Expose the report through `ContinuityDb<FileKernel>::file_store_health()`.

Extend `continuitydb inspect-kernel` with:

```json
"health": {
  "has_header": true,
  "legacy_raw_cells": 0,
  "checksum_free_records": 0,
  "canonical_records": 0,
  "compaction_recommended": false
}
```

The CLI should continue to fail before printing if open-time validation detects corruption. Health reports only validated readable stores.

## Testing

Kernel tests should cover:

- a new empty store reports headered, no legacy/checksum-free records, and no compaction recommendation;
- a legacy raw-cell store reports raw legacy cells and recommends compaction;
- a checksum-free typed envelope store reports checksum-free records and recommends compaction;
- compaction turns a readable legacy store into a canonical store with no compaction recommendation.

API and CLI tests should cover:

- the native API exposes the file-store health report;
- `inspect-kernel` includes the health object for a new store;
- `inspect-kernel` reports `compaction_recommended: true` for a legacy readable store.

## Non-Goals

- Do not add a deep offline verifier that scans stores independently of `FileKernel::open`.
- Do not add a generic `StorageKernel::health` method.
- Do not change corruption semantics or make corrupt stores partially inspectable.
- Do not detect external file mutation after a `FileKernel` has already been opened.

