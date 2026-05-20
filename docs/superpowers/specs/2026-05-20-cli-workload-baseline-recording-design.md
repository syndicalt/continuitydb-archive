# CLI Workload Baseline Recording Design

## Purpose

`continuitydb measure-workload` can emit measurement JSON, and `continuitydb-workload` can persist baseline records. The next step is to connect those pieces so operator and CI workflows can both see the current result and durably append it to a JSONL baseline file.

## Command Extension

Extend `measure-workload` with:

- `--baseline-path <path>`: optional JSONL file where the measurement baseline record is appended.
- `--label <label>`: optional baseline scenario label, defaulting to `default`.

When `--baseline-path` is omitted, behavior stays unchanged.

## Baseline Record

When `--baseline-path` is supplied:

- Run the same workload measurement.
- Convert the measurement to `WorkloadMeasurementSnapshot`.
- Append a `WorkloadBaselineRecord` with:
  - current UTC `recorded_at`,
  - supplied label,
  - measured kernel name,
  - snapshot.
- Include `baseline_path` and `baseline_label` fields in command JSON output.

Timing is observational; tests assert structure and counts, not exact elapsed values.

## Tests

Add CLI smoke tests for:

- Memory measurement writes one baseline record.
- File measurement writes one baseline record with kernel `file`.

## Roadmap Placement

Add Benchmark and Workload milestone 5: CLI baseline recording for workload measurements.
