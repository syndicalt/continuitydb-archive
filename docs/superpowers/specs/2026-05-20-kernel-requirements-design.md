# Kernel Requirements Design

## Purpose

Kernel capability introspection lets embedders inspect storage guarantees, but production callers still need a stable way to assert required guarantees before using a database for durable agent world-model state. This slice adds typed kernel requirements and native API enforcement.

## Problem

After `KernelCapabilities`, a caller can see that `MemoryKernel` is ephemeral and `FileKernel` is append-log durable. The caller still has to compare fields manually. That creates duplicated policy logic and makes future production kernels harder to adopt consistently.

The next storage boundary should let callers express requirements such as:

- any kernel is acceptable for correctness tests;
- a durable append-log kernel is required for persistent local use;
- a future indexed embedded kernel is required for production indexed storage.

## Design

Add `KernelRequirements` to `continuitydb-kernel`. It mirrors the observable guarantees in `KernelCapabilities`, but each boolean means "this guarantee is required." Durability is represented as a minimum durability level.

Preset constructors:

- `KernelRequirements::ephemeral()` requires only ephemeral-or-better append-only behavior.
- `KernelRequirements::durable_append_log()` requires append-log-or-better durability, append-only writes, explicit commit records, and durable flush.
- `KernelRequirements::indexed_embedded()` requires indexed-embedded durability, append-only writes, explicit commit records, durable flush, and persistent indexes.

`KernelCapabilities::satisfies(requirements)` returns `true` only when all requested guarantees are present and the actual durability is at least the requested minimum.

`ContinuityDb<K>` exposes:

- `kernel_satisfies(requirements) -> bool`;
- `ensure_kernel_requirements(requirements) -> Result<(), ContinuityError>`.

`ContinuityError` gains `KernelRequirementsNotMet { required, actual }` so callers can fail explicitly before performing production operations.

## Non-Goals

- Do not add runtime kernel selection or factory logic.
- Do not claim `FileKernel` has persistent indexes.
- Do not introduce a new storage engine.
- Do not encode performance requirements such as throughput, latency, or index selectivity.

## Data Flow

An embedder creates `ContinuityDb<K>`, then calls `ensure_kernel_requirements`. For a memory-backed test database, `KernelRequirements::ephemeral()` succeeds and durable requirements fail. For the current file-backed database, `KernelRequirements::durable_append_log()` succeeds and `KernelRequirements::indexed_embedded()` fails until a true persistent-index kernel exists.

## Error Handling

Requirement checks are deterministic and infallible until the API converts a failed check into `ContinuityError::KernelRequirementsNotMet`. The error carries both the required and actual capability data so callers can log, surface, or branch on the mismatch.

## Testing

Tests must prove:

- ephemeral capabilities satisfy ephemeral requirements;
- ephemeral capabilities do not satisfy durable append-log requirements;
- file append-log capabilities satisfy durable append-log requirements;
- file append-log capabilities do not satisfy indexed embedded requirements;
- the native API returns `Ok(())` for satisfied requirements and `KernelRequirementsNotMet` with required and actual data for failed requirements.

## Roadmap Impact

This creates the first typed production readiness gate for storage kernels. The future indexed embedded kernel can become usable through the same API by reporting stronger capabilities, without changing embedder code that already asks for `KernelRequirements::indexed_embedded()`.
