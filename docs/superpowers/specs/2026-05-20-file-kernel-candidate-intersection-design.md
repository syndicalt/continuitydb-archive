# File Kernel Candidate Intersection Design

## Problem

Smallest-candidate selection improves over fixed lookup priority, but it can still over-select. A narrow answerability index may include cells outside the requested scope, or a narrow temporal index may include cells rejected by another indexed constraint.

## Design

- Continue collecting candidate position vectors for every indexed `CellLookup` constraint.
- Use the smallest candidate vector as the ordered base.
- Convert the remaining vectors into membership sets.
- Retain only positions that appear in every indexed candidate set.
- Fall back to all cell positions when no indexed constraint is present.
- Keep final predicate filtering unchanged so index bugs cannot broaden public lookup semantics.

This remains a deterministic in-memory planner for the JSONL kernel. It does not yet estimate cost from histograms or materialize persistent index pages.

## Test

- Add a file-kernel test with a matching cell, broad scope-only cells, and a wrong-scope cell that shares the narrow answerability label.
- Assert candidate positions contain only the true indexed intersection and lookup results remain exact.
