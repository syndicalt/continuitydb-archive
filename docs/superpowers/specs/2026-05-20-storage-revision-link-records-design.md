# Storage Revision Link Records Design

## Purpose

ContinuityDB now has a core `RevisionLinkRecord`, but storage kernels still persist only StateCells and commit manifests. Accepted `LinkRevision` proposals therefore still need to be represented as operational StateCells. This slice adds native storage-kernel support for append-only revision-link records.

## Architecture

Extend `continuitydb-kernel` with:

- `RevisionLinkLookup` with optional `source`, `target`, and `kind` filters.
- `StorageKernel::append_revision_link`.
- `StorageKernel::list_revision_links`.

Implement the contract in:

- `continuitydb-memory`: store records in append order and filter deterministically.
- `FileKernel`: persist records as typed JSONL records with checksums, load them on open, preserve them through compaction, and expose filtered listing.

This slice does not yet change the native Steward `LinkRevision` application path. API-level adoption should follow once both kernels can store the records.

## Semantics

Revision links are append-only records. The storage layer records exactly what callers provide. It does not require endpoint StateCells to exist in this slice; endpoint validation remains an API/policy concern.

Lookup order is append order. All filters are conjunctive.

## Tests

Tests must prove:

- Memory kernel appends and filters revision links in append order.
- File kernel persists revision links across reopen.
- File kernel filters revision links by source, target, and kind.
- File kernel compaction preserves revision links.

## Roadmap Placement

Add Storage Kernel milestone 27: native revision-link record storage.
