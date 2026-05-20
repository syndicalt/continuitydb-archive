# CLI Commit Backup and Restore Design

## Goal

Expose file-backed commit backup and restore through the CLI using the native versioned commit export envelope.

## Current Behavior

The native API can export commit slices, import validated commit batches, and encode/decode those batches with a versioned JSON envelope. Operators can compact a file-backed store from the CLI, but they cannot yet export a portable commit backup file or restore one into another file-backed store without writing custom Rust code.

## Proposed Behavior

Add two CLI commands:

```text
continuitydb export-commits <store-path> <output-path>
continuitydb import-commits <store-path> <input-path>
```

`export-commits` opens the source store with `FileKernel`, exports all commits with `CommitManifestLookup::default()`, encodes the batch with `ContinuityDb::encode_commit_export_json`, writes the JSON envelope to `output-path`, and prints summary JSON:

```json
{
  "path": "/path/to/store.jsonl",
  "output": "/path/to/backup.json",
  "exported_commits": 1,
  "next_after": "commit-id-for-next-cursor"
}
```

`import-commits` opens or creates the target store with `FileKernel`, reads `input-path`, decodes and validates the JSON envelope with `ContinuityDb::decode_commit_export_json`, imports the decoded batch with `import_commit_batch`, and prints summary JSON:

```json
{
  "path": "/path/to/store.jsonl",
  "input": "/path/to/backup.json",
  "imported_commits": 1
}
```

## Error Handling

The commands should rely on existing typed errors and command failure behavior:

- invalid JSON or unsupported envelope format/version fails during decode;
- malformed commit export batches fail during import validation;
- duplicate target commits fail through the storage kernel;
- corrupt source or target stores fail through `FileKernel::open`.

No partial import rollback is added in this slice. Existing `import_commit_batch` validation must run before writes, and storage-kernel duplicate handling remains responsible for write-time rejection.

## Non-Goals

- Do not add streaming, compression, encryption, or incremental cursor options.
- Do not add native file helper APIs in this slice.
- Do not change the commit export envelope format.
- Do not add cross-store merge or conflict resolution behavior.

## Tests

Add CLI integration tests proving:

- `export-commits` writes a versioned JSON envelope for a file-backed store;
- `import-commits` restores that envelope into another file-backed store;
- the restored store exports the same commit data as the source;
- invalid envelope JSON causes `import-commits` to fail.
