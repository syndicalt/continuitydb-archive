# Revision Link Import Validation Design

## Purpose

Commit export batches now carry native revision-link records. Import validation checks that link endpoints exist or are included in the imported cells, but it does not yet reject duplicate link records in the incoming batch or link records already visible in the target store.

That leaves backup/replay behavior weaker than commit validation: cells and commits reject duplicates before mutation, while revision-link replay could append duplicated graph facts.

## Architecture

Keep the storage-kernel append API unchanged. Add native API import validation for revision links:

- reject duplicate `RevisionLinkRecord` values inside one `CommitExportBatch`,
- reject any imported revision link that is already visible in the target database,
- keep all checks inside `validate_commit_export_batch` so `import_commit_batch_with_summary` remains all-or-nothing before mutation.

Use the batch's first commit ID in `InvalidCommitExport` for malformed revision-link batches; use `CommitId::nil()` only for malformed link-only batches.

## Tests

Tests must prove:

- duplicate revision links inside a batch are rejected,
- pre-existing target revision links are rejected,
- rejection happens before imported cells or links become visible.

## Roadmap Placement

Add a Native API milestone for duplicate-safe revision-link import validation.
