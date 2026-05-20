# File Store Revision Link Status Design

## Purpose

Revision links are now native durable records with indexed lookup paths. File-store status still reports only visible StateCells, visible commits, and durable file size. That makes operators and embedders inspect a store without seeing whether it contains revision graph records, even though those records now affect audit, checkout, conflict traversal, and Steward application.

This slice adds revision-link visibility to file-store status.

## Architecture

Extend `FileKernelStatus` with `revision_link_count`. Populate it from the rebuilt `FileKernelIndex.revision_links` vector so the count reflects validated, visible records after open-time corruption and format checks.

Expose the field through:

- `FileKernel::status`,
- `ContinuityDb<FileKernel>::file_store_status`,
- `continuitydb inspect-kernel` JSON under `status.revision_link_count`.

No storage format change is required.

## Tests

Tests must prove:

- empty file stores report zero revision links,
- populated file stores report revision-link count together with cell and commit counts,
- native API status exposes the count,
- CLI inspection JSON includes the count.

## Roadmap Placement

Add Storage Kernel milestone 29 for revision-link-aware file-store status and update the existing CLI/native status descriptions.
