# Workload Measurement Harness Design

## Purpose

The deterministic workload generator gives ContinuityDB stable StateCell corpora. The next frontier step is a reusable measurement harness that can run those corpora through storage kernels and checkout without claiming benchmark superiority. This creates the evidence surface needed before designing a real indexed storage engine.

## Architecture

Extend `continuitydb-workload` with a small, storage-kernel-generic harness:

- `measure_ingest_and_checkout` accepts any mutable `StorageKernel`, a generated workload, a deterministic commit timestamp, and a `CheckoutRequest`.
- It appends the workload as one committed batch.
- It runs checkout against the same kernel.
- It returns operation counts, selected/alternative/frontier counts, token totals, and elapsed durations.

The harness depends on `continuitydb-kernel` and `continuitydb-checkout`. Tests use `continuitydb-memory` as the correctness kernel. This keeps workload generation and workload measurement together, while keeping actual engine implementations independent.

## Data Model

Add:

- `MeasuredOperation`: operation count and elapsed duration.
- `CheckoutMeasurement`: matched, selected, alternative, frontier, and selected-token counts.
- `WorkloadMeasurement`: workload summary plus ingest and checkout measurements.
- `MeasurementError`: typed wrapper around kernel and checkout failures.

Elapsed durations are recorded for benchmark usefulness, but tests only assert deterministic counts and semantic results. Timing values are observational and not used as correctness claims.

## Testing

Tests must prove:

- The harness ingests the generated workload into `MemoryKernel`.
- Checkout measurement reports selected, alternative, frontier, and token counts from the real checkout result.
- The harness surfaces duplicate-ingest kernel errors when a caller reuses a kernel containing the same deterministic workload IDs.

## Roadmap Placement

Add Benchmark and Workload milestone 2: storage-kernel-generic ingest and checkout measurement over deterministic workloads.
