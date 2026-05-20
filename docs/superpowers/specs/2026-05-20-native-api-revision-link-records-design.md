# Native API Revision Link Records Design

## Purpose

Storage kernels can now persist native `RevisionLinkRecord` values, but embedders still need to call the kernel directly. This slice exposes revision-link records through the native `ContinuityDb<K>` API and adds a native Steward `LinkRevision` application method that writes a real revision-link record.

## Architecture

Add native API methods:

- `record_revision_link_at(source, kind, target, recorded_at) -> RevisionLinkRecord`
- `list_revision_links(lookup) -> Vec<RevisionLinkRecord>`
- `apply_accepted_link_revision_record_proposal_at(record, recorded_at) -> Option<RevisionLinkRecord>`

The API validates source and target StateCells before appending a revision link. The storage kernel remains append-only and does not validate endpoints by itself.

This slice intentionally leaves the older `apply_accepted_link_revision_proposal_at` operational StateCell method and unified dispatch return type unchanged. That keeps existing callers stable while providing the native record path for embedders and for a future dispatcher migration.

## Tests

Tests must prove:

- Native API can append and list a revision link record.
- Accepted Steward `LinkRevision` writes a native `RevisionLinkRecord`.
- Rejected Steward `LinkRevision` writes no native revision-link record.
- Missing source or target still returns `CellNotFound`.

## Roadmap Placement

Add Native API milestone 36: native revision-link record operations.
