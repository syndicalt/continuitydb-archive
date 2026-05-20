# CLI Kernel Inspection Design

## Purpose

ContinuityDB now has typed kernel capabilities and requirement gates, but operators and embedders cannot exercise those gates from the command line. This slice adds a small CLI inspection command for file-backed stores so production-readiness checks are visible outside library tests.

## Problem

The CLI can compact file stores and move commit backups, but it cannot answer "what guarantees does this store provide?" or "does this store satisfy the durable append-log profile?" That forces downstream projects to write ad hoc readiness checks before trusting a store for persistent agent world-model state.

## Design

Add a CLI command:

```text
continuitydb inspect-kernel <store-path> [--require <profile>]
```

Profiles:

- `ephemeral`
- `durable-append-log`
- `indexed-embedded`

The command opens the path as a `FileKernel`, wraps it in `ContinuityDb`, reports capabilities as JSON, and optionally evaluates the requested profile through `ensure_kernel_requirements`.

Successful output includes:

- `path`
- `capabilities`
- `required`
- `satisfies`

`required` is `null` when no profile is supplied. For a file-backed store, `durable-append-log` succeeds and `indexed-embedded` fails because the current JSONL kernel has derived in-process indexes, not persistent indexes.

## Non-Goals

- Do not add a runtime kernel factory.
- Do not inspect memory kernels through the CLI.
- Do not claim the JSONL file kernel satisfies indexed embedded requirements.
- Do not add a new storage engine.

## Error Handling

Invalid profile values are rejected by clap before command execution. Store corruption still fails through `FileKernel::open`. Unsatisfied requirements return the existing `KernelRequirementsNotMet` error and a non-zero process status.

## Testing

Tests must prove:

- `inspect-kernel <path>` returns JSON with append-log durability for a file store.
- `inspect-kernel <path> --require durable-append-log` succeeds and reports `satisfies: true`.
- `inspect-kernel <path> --require indexed-embedded` fails for the current file store.

## Roadmap Impact

This makes the storage production-readiness boundary operational. It gives local users, CI jobs, and downstream projects a deterministic way to check whether a file-backed store meets the required kernel profile before running higher-level workflows.
