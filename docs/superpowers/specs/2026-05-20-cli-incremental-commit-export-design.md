# CLI Incremental Commit Export Design

## Problem

ContinuityDB already has cursor-based commit export in the native API through `CommitManifestLookup { after, limit }`, and CLI export output already reports `next_after`. Operators cannot yet feed that cursor back into `continuitydb export-commits`, so CLI backup and sync workflows are forced into full-store exports.

## Options Considered

1. Add a separate `export-commits-page` command.
2. Add `--after` and `--limit` flags to the existing `export-commits` command.
3. Keep cursoring native-only and require embedders to build their own operators.

Option 2 is the right next slice. It preserves the existing command, maps directly to the native API, and keeps the CLI surface small.

## Design

Extend `continuitydb export-commits <store-path> <output-path>` with:

```text
--after <commit-id>
--limit <count>
```

Both flags are optional. Without either flag, behavior remains unchanged. With `--limit`, the exported envelope contains at most that many commit slices. With `--after`, export starts after the supplied exclusive commit cursor. The JSON output continues to include `exported_commits` and `next_after`, so callers can loop until `exported_commits` is zero or no further page is desired.

`CommitId` needs stable string ergonomics so the same value serialized in CLI JSON can be passed back as `--after`. Implement `Display` and `FromStr` for `CommitId` using the underlying UUID textual format. Invalid cursor strings should fail at CLI argument parsing before opening or mutating a store.

## Data Flow

1. CLI parses optional `after: Option<CommitId>` and `limit: Option<usize>`.
2. CLI builds `CommitManifestLookup { after, limit }`.
3. CLI calls `ContinuityDb<FileKernel>::export_commits_json_file`.
4. The file helper writes a versioned commit export envelope containing only the selected page.
5. CLI prints path, output path, exported count, and next cursor.

## Error Handling

- Unknown `--after` commit IDs return the existing `KernelError::CommitNotFound` path.
- Invalid commit-id syntax is rejected by clap using `CommitId::from_str`.
- A zero limit is allowed and produces an empty export batch with `next_after: null`, matching the existing lookup behavior for empty selections.
- Existing file I/O and envelope encoding failures keep their current errors.

## Tests

- Core test: `CommitId` displays as a UUID string and parses back to the same value.
- Core test: invalid UUID text fails to parse as `CommitId`.
- CLI test: exporting with `--limit 1` writes one commit and prints a usable `next_after`.
- CLI test: exporting with `--after <next_after>` writes the following commit.
- CLI test: unknown `--after` returns failure.
- CLI test: invalid `--after` syntax returns failure.

## Roadmap Impact

- Add a CLI milestone for incremental cursor/limit commit export.
- Add a README current-scope bullet for CLI incremental commit export.
