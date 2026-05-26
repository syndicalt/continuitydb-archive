# Archived Project

ContinuityDB is archived as a failed research prototype.

The project produced useful engineering work and useful negative evidence, but
the original plan did not work out. The central claim, that Zaxy or similar
agent-memory systems required a specialized `StateCell` database primitive, was
not supported strongly enough after falsification work and comparison against
ordinary hybrid memory/retrieval approaches.

## Archive Decision

- No further ContinuityDB product development is planned.
- No production storage engine should be built from the current thesis.
- No academic paper should present ContinuityDB as a validated database
  breakthrough.
- Existing code and benchmark artifacts are retained only for audit, salvage,
  and historical context.

## What Remains Useful

- stale-belief and revision benchmark cases
- adversarial memory-task design
- context packet failure analysis
- evidence-retention and reproducibility habits
- the lesson that memory behavior must be measured before a new primitive is
  justified

## Future Direction

Future agent-memory work should start outside this repository. A better next
step would be an evaluation-first effort around real agent memory failures,
using existing products where appropriate:

- Zaxy for local developer memory/context workflows
- Eventloom for evented observability, replay, and portability
- AIegis for trust-boundary and memory-poisoning defense

ContinuityDB should stay closed unless new independent evidence demonstrates a
storage-shaped failure that existing retrieval, runtime, and provider memory
systems cannot solve.
