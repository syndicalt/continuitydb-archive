# CLI Workload Measurement Design

## Purpose

ContinuityDB now has deterministic workload generation and a storage-kernel-generic measurement harness. The next step is to expose that harness through the CLI so operators and future CI jobs can collect comparable ingest and checkout evidence for memory and file-backed kernels.

This command is not a formal benchmark suite and does not make performance claims. It emits deterministic counts plus observational elapsed nanoseconds so later benchmark automation can compare kernels against the same corpus.

## Command

Add:

```bash
continuitydb measure-workload --kernel memory --cells 8 --token-budget 400
continuitydb measure-workload --kernel file --store-path /tmp/workload.jsonl --cells 8 --token-budget 400
```

Parameters:

- `--kernel memory|file`, default `memory`.
- `--store-path <path>`, required only for `file`.
- `--cells <n>`, default `8`.
- `--token-budget <n>`, default `400`.
- `--frontier-every <n>`, default `3`.
- `--dependency-stride <n>`, default `2`.

The command uses stable workload defaults for anchor prefix, project scope, ID seed, and valid time.

## Output

The command prints JSON:

- `kernel`: selected kernel.
- `store_path`: file path for file-backed runs, otherwise null.
- `workload`: generated workload summary.
- `ingest`: operation count and elapsed nanoseconds.
- `checkout_operation`: operation count and elapsed nanoseconds.
- `checkout`: matched, selected, alternative, frontier, and selected-token counts.

## Errors

If `--kernel file` is used without `--store-path`, the command fails before running measurement. Kernel and checkout errors are surfaced through the existing CLI error path.

## Tests

Add CLI smoke tests for:

- Memory-kernel measurement JSON.
- File-kernel measurement JSON and durable file creation.
- Missing `--store-path` for file mode.

## Roadmap Placement

Add Benchmark and Workload milestone 3: CLI workload measurement for memory and file kernels.
