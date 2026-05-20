# File Kernel Corruption Diagnostics Design

## Goal

Make JSONL file-kernel corruption actionable by reporting the physical record line that failed decoding, header validation, or checksum validation.

## Current Behavior

`FileKernel::open` reads the JSONL log line-by-line, skips blank lines, and maps malformed records to `KernelError::StoreCorrupt`. This preserves safety but loses the operator's most useful diagnostic: which durable record is damaged.

## Proposed Behavior

Add a new kernel error variant:

```rust
KernelError::StoreCorruptRecord { line: usize }
```

The line value is the one-based physical line number in the JSONL file. Blank lines are still ignored as records, but they still count as physical lines for diagnostics.

`read_log_from_path` should return this variant for:

- invalid JSONL that is neither a current envelope record nor a legacy raw `StateCell`;
- unsupported header records;
- duplicate or late header records;
- cell or commit record checksum mismatches.

Semantic corruption detected after the log has been decoded into a `FileKernelLog` can continue returning the existing `StoreCorrupt` variant. Examples include an explicit commit manifest that references missing cells or mismatched commit IDs. Those errors are not tied to one decode failure line without adding a richer validation context.

## Non-Goals

- Do not change the JSONL durable format.
- Do not change legacy raw `StateCell` compatibility.
- Do not add path names or source error strings to `KernelError`; the first hardening step is deterministic line-addressed classification.
- Do not expand the public `StorageKernel` trait.

## Tests

Add file-kernel tests that prove:

- malformed JSON on line 2 returns `StoreCorruptRecord { line: 2 }`;
- unsupported header on line 1 returns `StoreCorruptRecord { line: 1 }`;
- a tampered checksummed record after a valid header returns the tampered record's line.

Keep existing broad `StoreCorrupt` tests for semantic corruption unless a test specifically covers decode-time corruption.
