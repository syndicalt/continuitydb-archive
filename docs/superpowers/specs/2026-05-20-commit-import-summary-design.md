# Commit Import Summary Design

## Problem

ContinuityDB can now export commit backup pages with `--after` and `--limit`. Each exported page carries `next_after`, but import APIs and the CLI only report how many commits were imported. That forces backup/sync callers to decode the backup envelope separately if they want to checkpoint the cursor after a successful import.

## Options Considered

1. Change `import_commit_batch` and `import_commits_json_file` to return a summary struct.
2. Add new summary-returning import methods and keep the existing count-returning methods stable.
3. Only change CLI output by decoding the backup file twice.

Option 2 is the safest next slice. It improves embedders and operators without breaking existing API callers. The existing methods remain count-returning conveniences.

## Design

Add:

```rust
pub struct CommitImportSummary {
    pub imported_commits: usize,
    pub next_after: Option<CommitId>,
}
```

Add generic API:

```rust
pub fn import_commit_batch_with_summary(
    &mut self,
    batch: CommitExportBatch,
) -> Result<CommitImportSummary, ContinuityError>
```

The method validates the batch once, imports the same way as `import_commit_batch`, and returns both the number of imported commit slices and the exported page cursor. `import_commit_batch` delegates to this method and returns `summary.imported_commits`.

Add file API:

```rust
pub fn import_commits_json_file_with_summary<P: AsRef<Path>>(
    &mut self,
    input_path: P,
) -> Result<CommitImportSummary, ContinuityError>
```

The existing `import_commits_json_file` delegates to the summary method and preserves its current return type.

Update CLI `import-commits` output for non-dry-run imports to include:

```json
"next_after": "..."
```

For empty imports, `next_after` is null. Dry-run output remains unchanged because it reports validation, not mutation. Existing `imported_commits` output remains unchanged.

## Error Handling

- Validation failures remain identical because the new method uses the same validator before mutation.
- Duplicate commit/cell errors remain unchanged.
- File I/O and invalid JSON errors remain unchanged.
- The summary cursor is copied from the decoded export batch and does not infer anything from the target store.

## Tests

- API test: summary import reports `imported_commits` and `next_after` and mutates the target.
- API test: count-returning import still returns the same count.
- File API test: summary import from a backup file reports `next_after`.
- CLI test: `import-commits` output includes the backup `next_after`.
- CLI dry-run test remains unchanged and should not include `imported_commits`.

## Roadmap Impact

- Add a native API milestone for commit import summaries.
- Add a CLI milestone note that import reports backup cursors for checkpointing.
- Add a README current-scope bullet for import cursor summaries.
