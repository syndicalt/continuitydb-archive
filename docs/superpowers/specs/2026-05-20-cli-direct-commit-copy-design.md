# CLI Direct Commit Copy Design

## Problem

The native API can now replay cursor-selected commit pages between two open databases with `copy_commits_from`. The CLI still requires operators to create an intermediate JSON export file and then import it into the target. That is useful for portable backups, but local file-backed sync should have a direct operational path.

## Options Considered

1. Add `copy-commits <source-store> <target-store>` to copy a cursor-selected page directly.
2. Extend `import-commits` to accept a source store path instead of an envelope path.
3. Keep operators on explicit export/import files only.

Option 1 is the right next slice. It keeps backup file workflows intact, exposes the native direct-copy API without overloading import semantics, and mirrors the `export-commits --after --limit` cursor model.

## Design

Add CLI command:

```text
continuitydb copy-commits <source-store-path> <target-store-path> [--after <commit-id>] [--limit <count>]
```

The command:

1. Opens the source file-backed database read-only from the API boundary perspective.
2. Opens or creates the target file-backed database mutably.
3. Calls `target.copy_commits_from(&source, CommitManifestLookup { after, limit })`.
4. Prints JSON:

```json
{
  "source": "...",
  "target": "...",
  "copied_commits": 1,
  "next_after": "..."
}
```

If the selected page is empty, `copied_commits` is `0` and `next_after` is `null`. Duplicate target commits and unknown source cursors use existing errors.

## Scope

This is a local file-backed operator command. It is not a network replication system, background sync process, conflict resolver, retry loop, checkpoint store, or merge policy.

## Tests

- Copying one limited page copies only the first commit and reports the cursor.
- Copying after the first cursor copies the next commit.
- Copying from an empty source reports zero copied commits.
- Copying a duplicate commit into a target fails and leaves the target with its existing commit only.
- Invalid `--after` syntax fails during argument parsing.

## Roadmap Impact

- Add a CLI milestone for direct local commit copy.
- Add a README current-scope bullet for CLI direct commit copy.
