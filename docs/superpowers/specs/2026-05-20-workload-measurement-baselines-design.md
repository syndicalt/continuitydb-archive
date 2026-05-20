# Workload Measurement Baselines Design

## Purpose

ContinuityDB can now generate deterministic workloads, measure ingest and checkout through kernels, and expose the result through the CLI. The next frontier step is durable baseline recording: measurement evidence should survive across runs so future storage-engine changes can compare against historical data instead of one-off terminal output.

This slice adds a JSONL baseline store to `continuitydb-workload`. It does not add pass/fail regression policy yet.

## Architecture

Extend `continuitydb-workload` with serializable measurement snapshots and an append-only JSONL file store:

- `WorkloadMeasurementSnapshot`: serializable copy of measurement counts and elapsed nanoseconds.
- `WorkloadBaselineRecord`: metadata plus one snapshot.
- `FileWorkloadBaselineStore`: path-backed JSONL append/list store.

The baseline store creates parent directories, appends one JSON record per line, lists records in file order, returns an empty list for a missing file, and reports malformed JSONL as a typed error.

The store belongs in `continuitydb-workload` because it records workload measurement evidence independent of the CLI and independent of a specific kernel implementation.

## Metadata

Each record includes:

- `recorded_at`: timestamp for when the measurement was captured.
- `label`: caller-provided scenario label.
- `kernel`: caller-provided kernel profile name.
- `snapshot`: serializable measurement data.

The snapshot converts `Duration` to elapsed nanoseconds so JSON does not depend on Rust duration internals.

## Tests

Tests must prove:

- Snapshot conversion preserves counts and elapsed nanoseconds.
- File baseline store appends and lists records in order.
- Missing baseline file lists as empty.
- Invalid JSONL returns a typed decode error with line number.

## Roadmap Placement

Add Benchmark and Workload milestone 4: durable workload measurement baseline records.
