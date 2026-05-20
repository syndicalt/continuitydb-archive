# File Kernel Revision Link Indexes Design

## Purpose

Native revision-link records are now persisted by the JSONL file kernel and surfaced through direct audit and checkout audit traces. Filtered revision-link listing still uses the append-order vector as the only access path. That keeps behavior correct, but it weakens the file kernel as the production-storage reference because source, target, and kind lookups are central to supersession, conflict, provenance, and audit traversal.

This slice adds derived in-process revision-link indexes to the file kernel while preserving the append-only JSONL log as the source of truth.

## Architecture

Extend `FileKernelIndex` with derived maps over revision-link vector positions:

- source StateCell ID to revision-link positions,
- target StateCell ID to revision-link positions,
- revision-link kind to revision-link positions,
- source plus kind to revision-link positions,
- target plus kind to revision-link positions,
- source plus target to revision-link positions,
- source plus target plus kind to revision-link positions.

Rebuild these maps from durable log records on open and maintain them after successful `append_revision_link`. Listing remains deterministic and append-order preserving. The lookup path should choose the most selective available index from the supplied `RevisionLinkLookup`, then apply the same source, target, and kind predicates as a correctness backstop.

The storage contract does not change. No persistent index files are added in this slice.

## Tests

Tests must prove:

- file-kernel revision-link indexes are rebuilt after reopening a durable store,
- revision-link indexes are updated immediately after append,
- lookup with source, target, and kind returns append-order filtered records through the same public API,
- existing persistence, compaction, and checksum behavior remains unchanged.

## Roadmap Placement

Add Storage Kernel milestone 28: file-kernel secondary indexes for revision-link lookups.
