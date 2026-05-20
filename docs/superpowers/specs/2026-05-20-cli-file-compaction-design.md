# CLI File Compaction Design

## Goal

Add a CLI command that compacts a JSONL file-backed ContinuityDB store into the canonical durable record format.

## Context

`FileKernel::compact` and `ContinuityDb<FileKernel>::compact_file_store` now exist, but operators still need a simple way to run compaction without writing Rust code. The CLI is currently thin and demo-oriented; this command is the first maintenance operation exposed from the command line.

## Design

Add a command:

```bash
continuitydb compact-file <path>
```

The command should:

1. Open the supplied path as a `FileKernel`.
2. Wrap it in `ContinuityDb<FileKernel>`.
3. Call `compact_file_store`.
4. Print a small deterministic JSON object:

```json
{
  "path": "/path/to/store.jsonl",
  "compacted": true
}
```

Opening a missing path may create a valid empty file-backed store, matching existing `FileKernel::open` behavior.

## Error Semantics

The command should use the existing CLI `Result` path. Corrupt stores or I/O failures should return a non-zero process exit through propagated errors.

## Tests

Add failing CLI tests before implementation:

- `compact-file` rewrites a legacy raw-cell file into header plus checksummed cell and commit records.
- `compact-file` prints JSON containing `compacted: true` and the path.
- `compact-file` fails for corrupt input.

Run the full verification gate before committing implementation:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

## Out of Scope

- Generic backend compaction.
- Automatic compaction policies.
- CLI ingest commands.
- CLI repair commands.
