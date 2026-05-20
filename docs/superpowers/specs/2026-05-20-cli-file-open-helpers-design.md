# CLI File Open Helpers Design

## Purpose

The native API now has file-backed open helpers, including requirement enforcement during construction. The CLI should use those helpers instead of hand-opening `FileKernel` directly so command behavior stays aligned with the embeddable API boundary.

## Problem

`continuitydb inspect-kernel`, `compact-file`, `export-commits`, and `import-commits` still call `FileKernel::open` directly and then wrap the result with `ContinuityDb::new`. That duplicates construction logic and lets the CLI drift from the native API path that embedders are expected to use.

## Design

Refactor the CLI file-backed commands:

- `inspect-kernel` without `--require` uses `ContinuityDb::open_file`.
- `inspect-kernel --require <profile>` uses `ContinuityDb::open_file_with_requirements`.
- `compact-file`, `export-commits`, and `import-commits` use `ContinuityDb::open_file`.

The visible JSON output remains unchanged. The failure mode for `inspect-kernel --require indexed-embedded` remains the typed requirement mismatch rendered through the CLI error display.

## Non-Goals

- Do not change command names or JSON output.
- Do not add new CLI flags.
- Do not change `FileKernel::open`.
- Do not remove direct `FileKernel` access from library users.

## Error Handling

All file-open errors and requirement mismatch errors continue to flow through `ContinuityError` and the CLI `Display` error path. This slice changes the construction path, not user-facing error semantics.

## Testing

Tests must prove the existing CLI behavior still works through the native open helpers:

- `inspect-kernel --require durable-append-log` succeeds.
- `inspect-kernel --require indexed-embedded` fails with the storage-requirement error.
- backup export/import and compaction behavior continue to pass through the same CLI tests.

## Roadmap Impact

This closes the loop between native API readiness gates and operational tooling. The CLI becomes a consumer of the same file-backed construction boundary embedders are expected to use.
