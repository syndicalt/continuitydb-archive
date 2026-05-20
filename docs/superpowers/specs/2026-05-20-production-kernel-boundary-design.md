# Production Kernel Boundary Design

## Purpose

ContinuityDB needs a storage boundary that can evolve from the current correctness and JSONL kernels toward a production embedded engine without forcing embedders to depend on concrete kernel types. The immediate slice adds typed kernel capability introspection. It does not introduce a new storage engine.

## Problem

`StorageKernel` currently defines behavior for appends, lookup, and commit manifest listing, but it does not describe the operational guarantees of a kernel. A caller can use `MemoryKernel` or `FileKernel`, but cannot ask whether the kernel is ephemeral, durable, append-log backed, persistently indexed, or capable of durable flush and compaction.

That absence makes the next production storage step harder:

- embedders cannot select a kernel based on required guarantees;
- tests cannot pin the difference between correctness kernels and durable kernels;
- a future indexed embedded kernel has no stable capability vocabulary to implement;
- the native API must keep exposing concrete-type-only helpers for every operational distinction.

## Design

Add a small, typed capability contract in `continuitydb-kernel`:

- `KernelDurability` classifies broad persistence level:
  - `Ephemeral` for in-process correctness kernels;
  - `AppendLog` for durable append-log kernels;
  - `IndexedEmbedded` reserved for future production kernels with persistent indexes.
- `KernelCapabilities` describes observable storage guarantees:
  - `durability`;
  - `append_only`;
  - `derived_indexes`;
  - `persistent_indexes`;
  - `explicit_commit_records`;
  - `durable_flush`;
  - `compaction`.

`StorageKernel` gains a default `capabilities()` method returning `KernelCapabilities::ephemeral()`. Concrete kernels override it when they provide stronger guarantees.

`MemoryKernel` reports ephemeral, append-only in-memory behavior with no durable flush, persistent indexes, explicit commit records, or compaction.

`FileKernel` reports append-log durability with derived in-process indexes, explicit commit records, durable flush, and compaction. It must not claim persistent indexes, because its indexes are rebuilt from the canonical JSONL log.

The native API exposes `ContinuityDb<K>::kernel_capabilities()` for any `K: StorageKernel`.

## Non-Goals

- Do not build the future indexed embedded kernel in this slice.
- Do not add runtime kernel selection or a factory.
- Do not move file-specific backup or compaction helpers into the generic trait.
- Do not model performance claims such as lookup complexity or throughput.

## Data Flow

An embedder creates `ContinuityDb<K>` as before. Before enabling features that require durable storage or production-index guarantees, it calls `kernel_capabilities()` and checks typed fields. This keeps product decisions at the API boundary while preserving deterministic storage semantics inside the kernel.

## Error Handling

Capability introspection is infallible. It returns static properties of the kernel implementation and must not perform I/O.

## Testing

Tests must prove:

- `MemoryKernel` reports ephemeral capabilities.
- `FileKernel` reports append-log capabilities with derived, non-persistent indexes.
- `ContinuityDb<K>::kernel_capabilities()` exposes the same kernel-reported values.
- Existing storage behavior remains unchanged.

## Roadmap Impact

This adds the first explicit production-kernel boundary. Future storage work can introduce an `IndexedEmbedded` kernel by implementing the same trait and claiming persistent indexes only when those indexes are durable source-of-truth-adjacent structures rather than rebuilt in-memory accelerators.
