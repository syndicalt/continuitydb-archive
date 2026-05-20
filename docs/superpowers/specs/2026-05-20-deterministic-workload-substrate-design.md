# Deterministic Workload Substrate Design

## Purpose

ContinuityDB needs repeatable world-model corpora before the real storage-engine track can be judged honestly. The first benchmark substrate should generate deterministic `StateCell` workloads that exercise the semantic dimensions ContinuityDB claims as first-class: anchors, scopes, evidence confidence, activation/frontier state, costs, utility signals, and dependencies.

This slice builds the corpus generator only. It does not add timing benchmarks, performance claims, Criterion, or a new storage engine. Those come after the corpus is stable.

## Architecture

Add a new workspace crate, `continuitydb-workload`, responsible for deterministic workload generation. It depends only on `continuitydb-core` and `chrono`.

The crate exposes:

- `WorkloadConfig`: validated generation parameters.
- `ContinuityWorkload`: generated cells plus a summary.
- `WorkloadSummary`: cell counts, frontier count, dependency count, and total token cost.
- `generate_world_model_workload`: deterministic generator returning `StateCell` values.

To make generated cell identifiers stable across runs, add `StateCellId::from_u128` in `continuitydb-core`. This is a small core primitive useful beyond tests: imports, fixtures, replay, and benchmark corpora all need deterministic identifiers without exposing the UUID internals.

## Workload Semantics

For cell index `i`, the generator creates:

- A stable ID derived from `id_seed + i`.
- A semantic anchor of `<anchor_prefix>:cell:<zero-padded-index>`.
- A project scope from config.
- One answerability question tied to the index.
- Text payload with stable content.
- Evidence source and citation tied to the index.
- Confidence cycling through bounded deterministic values.
- Token cost derived from the index.
- `Frontier` activation when `frontier_every` divides `i + 1`; otherwise `Active`.
- A `DependsOn` dependency to `i - dependency_stride` when the index is far enough into the corpus.

The generator preserves vector order as append order so benchmark runners can ingest cells without extra sorting.

## Validation

Invalid configs fail before generating cells:

- `cell_count` must be positive.
- `anchor_prefix` must be non-empty after trimming.
- `project_scope` must be non-empty after trimming.
- `frontier_every` must be positive.
- `dependency_stride` must be positive.

## Testing

Tests must prove:

- Deterministic IDs can be constructed from fixed integers.
- Two generated workloads from the same config are identical.
- Generated cells include frontier activations, dependencies, evidence, and token costs.
- Invalid configs return typed errors.

## Roadmap Placement

Add a new Benchmark and Workload Milestones section. The first milestone is deterministic world-model workload generation. Later milestones can add benchmark runners for memory kernel, file kernel, checkout packing, and future indexed storage engines.
