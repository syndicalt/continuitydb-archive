# ContinuityDB Initial Design

## Status

Approved for initial repository design. Implementation planning must follow this document and remain test-first.

## Thesis

ContinuityDB is a Rust-native, embeddable datastore for agent world models: context, beliefs, knowledge, evidence, uncertainty, and operational truth.

The goal is not to build an application-specific memory system. Agent runtimes such as Zaxy should eventually consume ContinuityDB through stable APIs, but the product itself is a foundational storage and query technology.

ContinuityDB's ambition:

> ContinuityDB should be to agent world models what relational databases were to business records: a native storage and query abstraction, not an app-layer memory framework.

## Differentiation

The closest adjacent system identified so far is Zep/Graphiti: a temporal knowledge graph/context-memory engine for AI agents. Graphiti validates that agent memory needs temporal knowledge, provenance, evolving relationships, and context retrieval. ContinuityDB should study it deeply, but it should not clone its abstraction.

ContinuityDB differentiates by making the primitive unit a StateCell rather than an entity, edge, memory block, chunk, or retrieval artifact. The database should own belief revision, supersession, utility-aware context packing, answerability, bitemporal validity, and provenance as native semantics.

Other adjacent systems inform the design without defining it:

- XTDB proves that bitemporality belongs in the database rather than in ad hoc application code.
- Datomic proves the value of immutable facts, history, and as-of database values.
- Letta proves that agents need actively managed state and context hierarchy.
- Cognee and Mem0 prove product demand for graph/vector/provenance memory layers, while leaving room for a lower-level datastore abstraction.

## Non-Goals

- Do not target a Zaxy-specific use case for the first milestone.
- Do not frame ContinuityDB as a wrapper around SQLite, LatticeDB, Graphiti, or any other backend.
- Do not build a general agent runtime.
- Do not build a managed cloud service before the embeddable engine is coherent.
- Do not implement shortcuts or demo-only behavior. Production code only.

## Core Primitive: StateCell

A StateCell is an append-only, evidence-backed, temporally-aware unit of operational truth. It is not a row, graph node, document, vector, event, or chat memory.

Every StateCell version must carry these dimensions:

- Identity: immutable ID plus semantic anchors.
- Time: valid time and system time, supporting as-of queries.
- Evidence: citations, confidence, source identity, trust signals, and lineage.
- Causality: dependency, derivation, supersession, and conflict links.
- Scope: personal, project, team, organization, global, or task-specific visibility.
- Answerability: the questions or task intents this cell can help answer.
- Activation: dormant, active, frontier, or retired lifecycle state.
- Utility: relevance, recency, decision impact, feedback, and learned usefulness.
- Cost: token, compute, materialization, and bandwidth estimates.
- Payload: structured properties, text, vectors, subgraphs, streams, or binary references.

StateCells are immutable by default. Updates append new versions and link to predecessors through explicit supersession or conflict relationships.

## Native Operations

ContinuityDB should expose library APIs first. Query syntax can come later once semantics stabilize.

Initial operations:

- `ingest`: append new evidence or source material.
- `revise`: create new StateCell versions, detect conflict, and link supersession.
- `checkout`: materialize the smallest sufficient continuity slice for a task under validity, confidence, scope, and token/economic constraints.
- `audit`: explain why a StateCell or checkout result exists, including evidence and derivation.
- `feedback`: record outcome signals that update future utility estimates.
- `watch`: expose active frontier changes relevant to a scope or task.
- `fork`: create hypothetical branches for simulation without contaminating the main truth lineage.

The first implementation plan should only include a minimal subset of these operations. The design must keep the full operation model visible so early choices do not block it.

## Architecture

ContinuityDB owns semantic behavior. Storage backends provide primitive persistence and indexing.

```text
ContinuityDB engine
  StateCell model
  revision and conflict policies
  checkout/materialization optimizer
  provenance and audit semantics
  frontier tracking
  token and utility accounting
  API and future query language

Storage kernel interface
  append log
  bitemporal lookup
  graph edge lookup
  vector/semantic lookup
  text lookup
  property lookup
  payload/blob access
  transaction boundary

Backend implementations
  in-memory correctness backend
  LatticeDB backend candidate
  SQLite/reference backend if useful later
  future native ContinuityDB kernel
```

The storage kernel boundary is mandatory. LatticeDB may become the preferred serious backend if it supplies graph, vector, text, stream, and ACID-like primitives, but it is not ContinuityDB's identity. SQLite may be useful for a reference backend or portability tests, but it must not define the architecture.

## Rust Stack

ContinuityDB should be implemented as a Rust workspace.

Planned crate layout:

```text
crates/
  continuitydb-core/      # StateCell, evidence, time, scope, confidence
  continuitydb-kernel/    # StorageKernel traits and query primitives
  continuitydb-memory/    # in-memory backend
  continuitydb-revision/  # conflict detection, supersession, belief policies
  continuitydb-checkout/  # context packing and utility optimizer
  continuitydb-frontier/  # active frontier tracking
  continuitydb-ffi/       # C-compatible ABI
  continuitydb-cli/       # inspect, ingest, checkout, audit
docs/
  research/
  superpowers/specs/
```

Rust is the engine language because it provides systems-level control, memory safety, strong domain modeling, concurrency, embeddability, C ABI support, and future bindings for Python, Node, and WASM.

## First Milestone

The first implementation milestone should establish the smallest production-quality core:

1. Rust workspace with strict linting and formatting.
2. Core StateCell domain model.
3. Storage kernel trait with explicit capability boundaries.
4. In-memory backend for correctness tests.
5. Append-only ingest path.
6. Basic revision links: predecessor, supersedes, conflicts-with, derives-from.
7. Basic checkout over in-memory data using deterministic filtering and scoring.
8. Audit output for a checked-out cell or slice.
9. CLI as a thin wrapper over library APIs.

The first milestone does not need a native query language, distributed storage, ML utility prediction, vector search, or LatticeDB integration. Those are later slices.

## Test-First Standard

Implementation must be test-first. Before production code for a behavior, add a failing test or executable specification that captures the expected behavior.

Required early test categories:

- StateCell identity and immutability.
- Valid-time and system-time behavior.
- Evidence and provenance preservation.
- Supersession and conflict links.
- Storage kernel contract conformance.
- In-memory backend append and lookup behavior.
- Checkout selection under scope, confidence, validity, and token budget.
- Audit trace completeness.

No hacks, demo-only bypasses, unchecked panics in library code, or behavior hidden in CLI-only paths.

## Open Research Questions

- What exact StateCell schema is minimal but not underpowered?
- How should confidence combine across independent and dependent evidence?
- Which conflict policies are built in, and which are user-defined?
- What is the formal meaning of "smallest sufficient continuity slice"?
- How should answerability be represented: tags, embeddings, predicates, examples, or learned models?
- How should token/economic cost be estimated before materialization?
- What storage kernel capabilities are required versus optional?
- How much of checkout should be deterministic before ML-based utility prediction is introduced?
- How should an embedded database Steward model propose revisions, conflicts, answerability, and frontier work while keeping deterministic policy in control of committed truth?

## Research Sources To Study First

- Zep/Graphiti: temporal context graph and agent memory architecture. See <https://github.com/getzep/graphiti> and <https://arxiv.org/abs/2501.13956>.
- XTDB: bitemporal database semantics. See <https://docs.xtdb.com/about/time-in-xtdb.html>.
- Datomic: immutable facts, history, and as-of queries. See <https://docs.datomic.com/datomic-overview.html>.
- Letta/MemGPT: agent-managed memory hierarchy. See <https://docs.letta.com/guides/core-concepts/stateful-agents>.
- Cognee: graph/vector/provenance AI memory pipelines. See <https://docs.cognee.ai/getting-started/introduction>.
- Mem0: production memory platform surface area. See <https://docs.mem0.ai/platform/overview>.

## Roadmap Addendum

The frontier roadmap lives in `docs/roadmap.md`. It adds an embedded database Steward model track and current small-model candidates for local, proposal-only stewardship.

## Approval Gate

This design is the starting point for implementation planning. Before code is scaffolded, the written spec should be reviewed and accepted. The next step after approval is a test-first implementation plan.
