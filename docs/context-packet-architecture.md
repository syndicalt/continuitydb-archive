# Context Packet Architecture

ContinuityDB checkout should be understood as packet architecture, not raw retrieval. `StateCell` remains the durable continuity-state substrate; `checkout(task,budget)` is the primary operation; `ContextPacket` is the bounded, task-facing result an agent can use without losing citations, lifecycle policy, uncertainty, revision context, trajectory applicability, or invalidation constraints.

This note records the acceptance direction for the packet-design lesson and the checkout semantics now being made explicit through packet planning.

## Pipeline

Packet compilation is a staged pipeline:

```text
candidate retrieval
  -> semantic filtering
  -> task-signal extraction
  -> packet planning
  -> packet assembly
  -> packet validation
  -> budget fitting
```

- Candidate retrieval finds possible `StateCell`s and revision/edge context through the storage and checkout query surfaces.
- Semantic filtering proves which candidates satisfy native constraints such as scope, lifecycle, uncertainty, invalidation, trajectory strategy, dependencies, and revision relationships.
- Task-signal extraction derives action, safety, change, readiness, debugging, reflection, trajectory-reuse, and token-pressure signals from the request and selected cells.
- Packet planning chooses the packet strategy, abstraction level, safety ordering, trajectory reuse, and budget allocation before model-facing lines are assembled.
- Packet assembly renders current operational truth, revision context, lifecycle guidance, uncertainty, context gaps, invalidation conditions, trajectory lessons, dependencies, and citations into structured packet entries.
- Packet validation rejects retained artifacts that drop required provenance, citations, compiler metadata, lifecycle policy, edge context, trajectory applicability, invalidation, or bounded scores.
- Budget fitting trims or orders packet lines under token pressure while preserving hard gates before lower-priority semantic prose.

Retrieval and compilation are separate phases. Retrieval answers "which committed state is eligible?" Compilation answers "what packet should an agent see for this task?" A storage-correct checkout can still fail if it compiles a packet that hides lifecycle safety, drops a hard invalidation, or surfaces an inapplicable trajectory lesson.

## Acceptance Principles

Packet tests are architectural acceptance tests. They should assert behavior under adversarial tasks, not only type presence or serializer shape.

- Lifecycle and uncertainty override raw salience. `DoNotUseForAnswer`, `VerifyBeforeUse`, uncertainty, calibration, and misuse-risk guidance must survive tight budgets ahead of ordinary semantic claims.
- Trajectory applicability and invalidation are hard gates. A high-confidence rollout lesson is not reusable when its applicability conditions do not match or its invalidation conditions are already satisfied by the task.
- Revision and supersession must remain model-facing. A packet that retrieves the current cell but hides the superseding correction can preserve storage semantics while still encouraging stale behavior.
- Packet evidence must be inspectable. Strategy, compilation policy, abstraction level, reason tags, evidence locators, citations, origin, dependency context, revision context, lifecycle policy, and selected-cell rationale are part of the acceptance surface.
- Raw baseline behavior remains a diagnostic and compatibility boundary, not proof that packet architecture is sufficient.

Historical adversarial and benchmark reports are retained artifacts, not current proof for the simplified product model. They should inform packet-design questions only when they are refreshed against the current StateCell -> checkout(task,budget) -> ContextPacket contract.

The architectural signal remains: packet acceptance must validate planning and compilation decisions separately from candidate retrieval.

## Current Design Direction

The packet planner should become an explicit layer between task-signal extraction and packet assembly. It should rank or gate competing compiler obligations before rendering lines, especially when safety, revision, scavenging, trajectory reuse, audit, and token pressure all apply.

The planner does not make models trusted writers. Model-assisted packet lines remain proposals that must cite selected evidence, target explicit StateCells, fit the token budget, preserve required metadata, and pass deterministic validation before checkout may materialize them. If a proposal is absent or invalid, checkout uses the automatic deterministic compiler.
