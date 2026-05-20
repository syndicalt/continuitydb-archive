# CLI Workload Artifact Replay Design

## Context

Workload artifact bundles now include deterministic `workload-cells.json` and `checkout-request.json` fixtures. That makes archives inspectable, but operators still need a CLI path that consumes those fixtures and re-executes the workload against a selected kernel without regenerating the corpus.

## Decision

Add `replay-workload --artifact-dir <dir> --kernel <memory|file> [--store-path <path>]`.

The command reads the versioned cells and checkout request artifacts, validates their format versions, appends the archived StateCells at the deterministic workload commit timestamp, executes the archived checkout request, and emits replay JSON with workload counts, ingest counts, checkout counts, fixture paths, fixture fingerprints, and file-kernel lookup-plan diagnostics when replaying against the file kernel.

This is a replay and diagnostic command. It does not record baselines or compare against prior measurements.

## Non-goals

- Do not add baseline comparison to replay in this slice.
- Do not add replay support for arbitrary custom fixture formats.
- Do not mutate anything except the explicitly selected replay file store when `--kernel file` is used.
