# Revision Link Commit Export Design

## Purpose

Native revision-link records now participate in audit, checkout, store status, and file-kernel lookup. Commit export, import, direct copy, and JSON backup still move only commit manifests and StateCells. A backup restored through those paths can therefore lose supersession, conflict, derivation, and predecessor graph records even though the cell history itself is present.

This slice makes commit export batches carry source-owned native revision links.

## Architecture

Extend `CommitExportBatch` with `revision_links: Vec<RevisionLinkRecord>`.

When exporting a cursor-selected commit page:

- materialize the selected commit slices exactly as today,
- collect the exported StateCell IDs,
- include revision links whose `source` is one of the exported IDs,
- preserve deterministic order by scanning exported cell IDs in commit/cell order and appending each source-side link in storage order,
- deduplicate records that may appear more than once.

Import validation must verify that every revision link endpoint is either already present in the target store or present in the imported batch. Import must append cells first, then revision links, so links can point to newly imported cells. Direct copy and JSON backup/restore should use the same batch path.

This preserves full backups and incremental successor-to-predecessor sync without importing unrelated graph records for cells outside the selected page.

## Tests

Tests must prove:

- native export batches include source-owned revision links,
- encoded JSON envelopes round trip revision links,
- imported batches restore revision links,
- direct copy preserves revision links,
- CLI export/import restores revision links through the backup file.

## Roadmap Placement

Add Native API and CLI milestones for revision-link-aware commit export/import/copy.
