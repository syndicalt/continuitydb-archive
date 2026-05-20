# Commit Import Dry Run Design

## Purpose

Expose non-mutating validation for commit import batches and commit backup files.

ContinuityDB already validates import batches internally before mutating a target store, but embedders and operators cannot run that validation as a separate CI/sync step. A dry-run path closes that gap: callers can prove that a backup is structurally valid, preserves commit and cell identities, and will not collide with the target before choosing to import it.

## Design Options Considered

1. Add a boolean `dry_run` argument to existing import methods.
   - Pros: fewer methods.
   - Cons: boolean mode switches are easy to misuse and make mutation behavior less obvious.

2. Add explicit `validate_*` methods and CLI `--dry-run`.
   - Pros: clear non-mutating API boundary, reuses existing validation logic, and keeps import mutation explicit.
   - Cons: adds a small amount of API surface.

3. Only add CLI validation.
   - Pros: enough for operators.
   - Cons: Rust embedders would still duplicate file decoding plus validation orchestration.

Recommended approach: option 2. Add explicit validation methods to the native API and a CLI dry-run flag that delegates to those methods.

## Native API

Add a public summary:

```rust
pub struct CommitImportValidation {
    pub valid_commits: usize,
}
```

Add generic method:

```rust
pub fn validate_commit_import(
    &self,
    batch: &CommitExportBatch,
) -> Result<CommitImportValidation, ContinuityError>
```

This method should call the existing internal `validate_commit_export_batch` and return the number of slices that would import. It must not mutate the kernel.

Add file helper:

```rust
pub fn validate_commits_json_file<P: AsRef<Path>>(
    &self,
    input_path: P,
) -> Result<CommitImportValidation, ContinuityError>
```

This method reads and decodes the versioned commit export envelope, then delegates to `validate_commit_import`.

## CLI

Extend:

```text
continuitydb import-commits <store-path> <input-path> --dry-run
```

When `--dry-run` is omitted, behavior is unchanged.

When set, the command should:

- open the target store;
- validate the backup file without mutation;
- print JSON with `dry_run: true`, `valid_commits`, and no `imported_commits` mutation count.

The command should return nonzero on the same validation failures that a real import would reject.

## Testing

API tests:

- validating a decoded commit batch reports the commit count and leaves target commit list empty;
- validating a backup JSON file reports the commit count and leaves target commit list empty;
- validating against a target that already has the commit returns the same duplicate commit error as import.

CLI tests:

- `import-commits --dry-run` reports `dry_run: true` and `valid_commits`;
- after CLI dry-run, the target store still has no imported commits;
- dry-run against an invalid envelope fails.

## Non-Goals

- Do not change import mutation semantics.
- Do not add network sync or background replication.
- Do not add cryptographic signatures in this slice.
- Do not validate files without opening a target database; collision checks are target-dependent.

