# File Kernel Activation and Dependency Indexes Design

## Goal

Extend the durable file kernel's derived in-process indexes to activation state and dependency target/kind filters, so more agent-world-model checkout paths can start from indexed candidates.

## Current Behavior

`FileKernelIndex` now indexes cell IDs, semantic anchors, answerability questions, evidence sources, commit IDs, and manifests. `CellLookup.activation`, `dependency_target`, and `dependency_kind` are supported by `FileKernel::lookup_cells`, but they are only applied in the final predicate pass unless another indexed constraint has already narrowed the candidate set.

## Proposed Behavior

Add derived indexes to `FileKernelIndex`:

```rust
activations: HashMap<ActivationState, Vec<usize>>
dependency_targets: HashMap<StateCellId, Vec<usize>>
dependency_target_kinds: HashMap<(StateCellId, CellDependencyKind), Vec<usize>>
```

During `FileKernelIndex::insert`, record the cell position under its activation state, every dependency target, and every dependency target/kind pair.

Update `FileKernel::lookup_cells` candidate selection order:

1. `cell_id`
2. `semantic_anchor`
3. `commit_id`
4. `answerability_question`
5. `evidence_source`
6. `activation`
7. `dependency_target + dependency_kind`
8. `dependency_target`
9. full cell scan

Keep the final predicate chain unchanged. This preserves correctness for combined filters such as activation plus evidence source, dependency target plus valid time, or dependency target/kind plus minimum confidence.

## Persistence Model

No new durable records or index files are added. These indexes are derived from the JSONL log on open and updated only after a durable append succeeds.

## Error Handling

No new errors are required.

## Non-Goals

- Do not add temporal, scope, confidence, vector, or text indexes in this slice.
- Do not change lookup result ordering.
- Do not persist indexes to disk.
- Do not change the `StorageKernel` trait.

## Tests

Add tests proving:

- activation indexes are rebuilt on reopen and updated after append;
- dependency target/kind indexes are rebuilt on reopen and updated after append;
- existing activation and dependency lookup behavior remains correct.
