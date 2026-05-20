# Workload Baseline Regression Design

## Purpose

Workload measurements can now be recorded as JSONL baselines. The next frontier step is deterministic comparison: a new measurement should be compared against the latest matching baseline so future storage-engine work can detect semantic or performance regressions before making claims.

This slice adds library-level comparison only. CLI enforcement can follow once the core comparison semantics are stable.

## Architecture

Extend `continuitydb-workload` with:

- `FileWorkloadBaselineStore::latest_matching(label, kernel)`.
- `WorkloadBaselineComparison`.
- `WorkloadBaselineRegression`.
- `compare_workload_snapshot_to_baseline`.

Latest matching baseline means the record with the newest `recorded_at` among records with the same label and kernel. If timestamps tie, the later JSONL record wins because it was observed later.

## Comparison Rules

The comparator treats deterministic counts as exact invariants:

- workload cell count
- workload frontier count
- workload dependency count
- checkout matched count
- checkout selected count
- checkout alternative count
- checkout frontier count
- checkout selected token count
- ingest operation count
- checkout operation count

Elapsed times are compared with a caller-provided tolerance percentage. A timing regression is reported only when current elapsed nanoseconds are greater than `baseline + tolerance`.

## Tests

Tests must prove:

- Latest matching baseline lookup ignores nonmatching label/kernel records and chooses newest matching record.
- Equal snapshots pass comparison.
- Deterministic count changes fail comparison.
- Elapsed growth within tolerance passes, while elapsed growth beyond tolerance fails.

## Roadmap Placement

Add Benchmark and Workload milestone 6: workload baseline regression comparison.
