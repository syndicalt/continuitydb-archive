# File Open Requirements Design

## Purpose

Embedders can now inspect and enforce kernel requirements after constructing `ContinuityDb<K>`, but file-backed users still have to remember the sequence: open `FileKernel`, wrap it in `ContinuityDb`, then call `ensure_kernel_requirements`. This slice adds a native file-open helper that performs the requirement gate during construction.

## Problem

The production-readiness gate is useful only if embedders apply it consistently. The current API makes the check optional and separate from opening the file-backed store. That is easy to forget in applications that need durable local world-model state.

## Design

Add two specialized constructors on `ContinuityDb<FileKernel>`:

- `open_file(path) -> Result<ContinuityDb<FileKernel>, ContinuityError>`
- `open_file_with_requirements(path, requirements) -> Result<ContinuityDb<FileKernel>, ContinuityError>`

`open_file` is a convenience wrapper for existing file-backed construction and does not enforce any stronger profile.

`open_file_with_requirements` opens the `FileKernel`, wraps it in `ContinuityDb`, calls `ensure_kernel_requirements`, and returns the database only when the file-backed kernel satisfies the requested profile.

For the current file-backed JSONL kernel:

- `KernelRequirements::durable_append_log()` succeeds.
- `KernelRequirements::indexed_embedded()` fails with `ContinuityError::KernelRequirementsNotMet`.

## Non-Goals

- Do not add a generic runtime kernel factory.
- Do not change `FileKernel::open` behavior.
- Do not claim the current file kernel has persistent indexes.
- Do not add a new storage engine.

## Error Handling

File opening errors propagate through `ContinuityError::Kernel`. Requirement mismatches return the existing `ContinuityError::KernelRequirementsNotMet` with the requested and actual profiles.

## Testing

Tests must prove:

- `open_file` opens a usable file-backed database.
- `open_file_with_requirements` accepts durable append-log requirements for the current file kernel.
- `open_file_with_requirements` rejects indexed embedded requirements for the current file kernel and reports the actual append-log capabilities.

## Roadmap Impact

This turns the storage requirement gate into an embeddable construction boundary. It gives applications a single safe entrypoint for durable local stores while preserving the future path where a true indexed embedded kernel can satisfy the stronger profile.
