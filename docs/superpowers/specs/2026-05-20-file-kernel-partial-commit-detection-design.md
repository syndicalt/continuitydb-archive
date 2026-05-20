# File Kernel Partial Commit Detection Design

## Goal

Prevent a crash-truncated JSONL append from becoming visible as a reconstructed commit when reopening a current-format file-kernel store.

## Current Behavior

The file kernel writes a current-format append as:

```text
header
cell
cell
commit
```

If a process or machine dies after one or more `cell` records are written but before the matching `commit` record reaches disk, reopening currently rebuilds a commit manifest from those cells. That makes a partial append visible as committed truth.

Legacy raw `StateCell` logs do not have explicit commit records, so they still need manifest reconstruction for backward compatibility.

## Proposed Behavior

Treat the versioned file-kernel header as the boundary for current-format semantics:

- Headered logs require every visible non-empty commit group to have an explicit commit manifest record.
- Headerless logs remain legacy-compatible and may reconstruct manifests from raw cells.
- Headered checksum-free envelope records remain readable if they include explicit commit records.
- A headered log with cells but no matching commit record is rejected as `KernelError::StoreCorrupt`.

This does not make the multi-line append physically atomic. It prevents a crash-truncated append from being promoted into committed database state on reopen. Future storage work can replace multi-line append groups with a single frame or indexed segment format.

## Non-Goals

- Do not change the durable JSONL record format in this slice.
- Do not add recovery or truncation repair.
- Do not reject headerless legacy raw logs.
- Do not change manifest ordering semantics.

## Tests

Add tests that prove:

- A headered log with a cell record and no commit record is rejected.
- A headered log with only the first cell of a two-cell commit and no commit record is rejected.
- Headerless legacy raw cell logs still reopen and reconstruct commit manifests.
