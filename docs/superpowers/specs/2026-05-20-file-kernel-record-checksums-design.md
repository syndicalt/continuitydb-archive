# File Kernel Record Checksums Design

## Goal

Add per-record checksums to new JSONL file-kernel cell and commit records so the durable log can detect accidental payload corruption during reopen.

## Context

The JSONL file kernel now has typed records, explicit commit records, and a versioned header. It can distinguish supported stores from future unsupported formats, but a syntactically valid edited or corrupted cell/commit record can still be accepted if its fields decode and cross-record invariants happen to pass.

Checksums give the append log a local integrity signal before larger storage work such as snapshots, compaction, or indexed sidecars.

## Design

Add a stable checksum field to new `cell` and `commit` records:

```json
{"type":"cell","cell":{...},"checksum":"continuitydb-fnv1a64:<hex>"}
{"type":"commit","manifest":{...},"checksum":"continuitydb-fnv1a64:<hex>"}
```

The checksum is computed over the canonical JSON bytes of the record payload:

- for `cell`, `serde_json::to_vec(cell)`;
- for `commit`, `serde_json::to_vec(manifest)`.

Use a small internal deterministic FNV-1a 64-bit checksum implementation. This is not a cryptographic guarantee, but it is sufficient for accidental corruption detection without adding a dependency or changing the storage API. The checksum string includes the algorithm name so a future format can introduce stronger hashes.

## Read Semantics

`FileKernel::open` should:

- validate checksums when a `cell` or `commit` envelope includes one;
- reject checksum mismatches as `KernelError::StoreCorrupt`;
- continue accepting legacy envelope records without checksums;
- continue accepting legacy raw `StateCell` lines.

The `header` record is not checksummed in this slice. Its format/version fields are already validated directly.

## Write Semantics

New writes should emit checksummed cell and commit records. The checksum is computed after stamping cells and after creating the commit manifest.

## Tests

Add failing tests before implementation:

- New cell and commit records include checksum fields with the expected algorithm prefix.
- Reopen accepts checksummed records written by the file kernel.
- Reopen rejects a cell record whose payload is changed without updating checksum.
- Reopen rejects a commit record whose manifest is changed without updating checksum.
- Legacy checksum-free envelope records still open.

Run the full verification gate before committing implementation:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

## Out of Scope

- Cryptographic tamper resistance.
- Checksumming the header.
- Whole-file checksums.
- Fsync or crash recovery.
- Compaction or repair.
