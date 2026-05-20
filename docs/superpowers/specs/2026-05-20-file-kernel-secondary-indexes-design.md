# File Kernel Secondary Indexes Design

## Goal

Extend the durable file kernel's deterministic in-process indexes beyond ID, semantic anchor, and commit ID so answerability-question and evidence-source lookups do not need to start from the full cell set.

## Current Behavior

`FileKernelIndex` is rebuilt from the JSONL log on open and maintained after successful appends. It currently indexes cell IDs, semantic anchors, commit IDs, and commit manifests. `CellLookup.answerability_question` and `CellLookup.evidence_source` are supported, but `FileKernel::lookup_cells` only applies those constraints during the final predicate pass unless another indexed constraint has already reduced the candidate set.

## Proposed Behavior

Add two secondary indexes to `FileKernelIndex`:

```rust
answerability_questions: HashMap<String, Vec<usize>>
evidence_sources: HashMap<String, Vec<usize>>
```

During `FileKernelIndex::insert`, record each cell position under every normalized answerability question and every evidence source ID. Because `Answerability::questions()` already exposes normalized questions and `SourceId::as_str()` exposes stable source text, the indexes should use those exact strings.

Update `FileKernel::lookup_cells` candidate selection so it can begin from these indexes when no more selective ID, anchor, or commit constraint is present:

1. `cell_id`
2. `semantic_anchor`
3. `commit_id`
4. `answerability_question`
5. `evidence_source`
6. full cell scan

Keep the existing final predicate chain unchanged. That preserves correctness for combined constraints such as answerability plus evidence source, answerability plus confidence, or evidence source plus valid time.

## Persistence Model

No new on-disk record format is required. The secondary indexes remain derived state rebuilt from the canonical append log on open and updated only after durable append succeeds.

## Error Handling

No new error variants are needed. Duplicate cell protection and corruption handling remain unchanged.

## Non-Goals

- Do not add a persistent index file.
- Do not add minimum-confidence, scope, activation, dependency, or temporal indexes in this slice.
- Do not change lookup result ordering; indexed lookups must preserve append/commit visibility order.
- Do not change the `StorageKernel` trait.

## Tests

Add kernel tests proving:

- answerability-question indexes are rebuilt on reopen and point to the expected cells;
- answerability-question indexes are updated after appending to an open file kernel;
- evidence-source indexes are rebuilt on reopen and point to the expected cells;
- evidence-source indexes are updated after appending to an open file kernel;
- existing lookup behavior for answerability/evidence filters remains correct.
