# Native Commit Backup File Helpers Design

## Goal

Expose commit backup and restore file helpers through the native `ContinuityDb<FileKernel>` API so embedders can use the same versioned envelope workflow as the CLI without reimplementing file I/O and envelope handling.

## Current Behavior

The native API can export commit batches, encode/decode versioned JSON envelopes, and import validated batches. The CLI now composes those pieces with `fs::read` and `fs::write`, but Rust embedders must still duplicate that orchestration to write a portable backup file or restore one.

## Proposed Behavior

Add a small summary type:

```rust
pub struct CommitExportFileSummary {
    pub exported_commits: usize,
    pub next_after: Option<CommitId>,
}
```

Add file helpers on `ContinuityDb<FileKernel>`:

```rust
pub fn export_commits_json_file<P: AsRef<std::path::Path>>(
    &self,
    lookup: CommitManifestLookup,
    output_path: P,
) -> Result<CommitExportFileSummary, ContinuityError>

pub fn import_commits_json_file<P: AsRef<std::path::Path>>(
    &mut self,
    input_path: P,
) -> Result<usize, ContinuityError>
```

`export_commits_json_file` exports the selected commit batch, encodes the versioned JSON envelope, writes it to `output_path`, and returns the commit count plus next cursor. `import_commits_json_file` reads `input_path`, validates the versioned envelope with existing decode logic, and imports the decoded batch with existing import validation.

Add a file I/O error variant:

```rust
ContinuityError::CommitExportFileIo
```

The variant should be deterministic and comparable in tests. It does not need to carry the underlying `std::io::Error` in this slice.

## CLI Integration

Update the CLI export/import commands to call the native file helpers instead of directly performing envelope file I/O. The CLI remains responsible for opening `FileKernel` and printing path-oriented summary JSON.

## Error Handling

- file read/write failures return `CommitExportFileIo`;
- invalid JSON or unsupported envelope version returns the existing envelope errors;
- malformed or duplicate import batches continue to use existing import validation and kernel errors.

## Non-Goals

- Do not add compression, encryption, streaming, or chunked cursor export.
- Do not add a generic helper for every storage kernel; this is intentionally file-backed.
- Do not change the versioned envelope format.
- Do not add rollback beyond existing pre-write import validation.

## Tests

Add API tests proving:

- exporting a committed file-backed store writes a valid versioned envelope file and returns the expected summary;
- importing that file into another file-backed store recreates the same exported commit batch;
- importing invalid envelope JSON through the file helper fails with `CommitExportJson`;
- exporting to a missing parent directory fails with `CommitExportFileIo`;
- CLI backup/restore tests still pass through the refactored native helpers.
