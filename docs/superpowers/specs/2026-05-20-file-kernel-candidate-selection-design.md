# File Kernel Candidate Selection Design

## Problem

The JSONL file kernel now maintains derived indexes for most `CellLookup` constraints, but lookup still chooses candidates through a fixed priority chain. When a broad constraint appears earlier than a narrow constraint, lookup starts from the broader candidate set and only narrows later through final predicates.

## Design

- Add `FileKernelIndex::candidate_positions(&CellLookup)`.
- Collect candidate position vectors for every indexed constraint present in the lookup.
- Choose the smallest candidate vector as the starting set.
- Fall back to all cell positions when no indexed constraint is present.
- Keep final predicate filtering unchanged so semantics remain exact.

This is not a full intersection planner yet. It is the smallest production step from indexed lookup toward query planning.

## Test

- Add a file-kernel test where broad scope and narrow answerability constraints are both present.
- Assert the chosen candidate positions come from the narrower answerability index and lookup results remain correct.
