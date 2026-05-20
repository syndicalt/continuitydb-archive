# Duplicate Revision-Link Storage Design

## Goal

Make native revision-link uniqueness a storage-kernel invariant.

## Context

ContinuityDB now stores revision links as first-class append-only records. Import validation rejects duplicate incoming revision-link records and links already present in the target, but direct kernel/API append paths can still append the same `RevisionLinkRecord` twice.

Revision links are operational truth about StateCell version relationships. Like immutable StateCell IDs and commit manifests, exact duplicate revision-link records should be rejected before mutation so every caller observes the same invariant whether links arrive through import, native API calls, Steward proposal application, or direct kernel use.

## Architecture

- Add `KernelError::DuplicateRevisionLink`.
- `MemoryKernel::append_revision_link` rejects an exact duplicate before pushing.
- `FileKernel::append_revision_link` rejects an exact duplicate before writing the durable JSONL record.
- `FileKernelIndex::insert_revision_link` rejects duplicates during rebuild so a durable log containing duplicate native revision-link records does not reopen as valid state.
- Native API callers naturally receive `ContinuityError::Kernel(KernelError::DuplicateRevisionLink)`.

Exact equality includes source, target, kind, and recorded system time. Same source/target/kind at a different recorded time remains a distinct append-only assertion.

## Non-Goals

- This does not add semantic conflict resolution between different revision-link kinds.
- This does not require endpoint validation at the storage-kernel layer.
- This does not add a hash index for revision-link equality; linear duplicate checks are acceptable for this invariant slice.

## Verification

- Memory kernel rejects duplicate revision-link appends without increasing visible link count.
- File kernel rejects duplicate revision-link appends before writing another JSONL line.
- File kernel rejects duplicate durable revision-link records during reopen.
- Native API duplicate record attempts surface the kernel duplicate error and preserve a single visible link.

## Roadmap Impact

- Storage Kernel: duplicate revision-link append rejection across memory and file kernels, including duplicate durable-log detection.
- Native API: duplicate revision-link record attempts surface a typed storage-kernel duplicate error.
