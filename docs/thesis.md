# ContinuityDB Thesis

> Status: superseded / failed thesis.
>
> This document is retained as historical context. The original thesis, that
> agent memory requires a specialized StateCell database primitive, did not
> survive falsification. Future work should start from the postmortem and from
> independent frontier-memory evaluation, not from extending this thesis. See
> [Project Postmortem](project-postmortem.md).

## Thesis Statement

ContinuityDB is a native datastore for continuity state. It treats context, beliefs, evidence, uncertainty, revision history, and operational truth as database semantics instead of transient prompt material, vector-search side effects, or application-layer memory conventions.

The core claim is that LLM memory should not be a growing transcript or an opaque retrieval cache. It should be a committed, queryable, evidence-backed continuity layer where every remembered state has provenance, temporal boundaries, uncertainty, revision relationships, and deterministic checkout behavior.

ContinuityDB should be to agent world models what relational databases became for business records: the durable substrate that lets higher-level systems reason over state without reinventing storage, consistency, audit, and query semantics in every application.

## What We Are Proving

ContinuityDB is trying to prove five concrete claims:

1. Agent memory needs a native state primitive.

   A `StateCell` is not a document chunk, graph node, vector, event, row, or chat message. It is an append-only, evidence-backed, temporally-aware unit of operational truth. This lets the database represent what an agent currently believes, why it believes it, when that belief is valid, what superseded it, what conflicts with it, and what evidence supports it.

2. Checkout should materialize continuity, not merely similarity.

   `checkout(task,budget)` should select a bounded continuity slice from structured state: current frontier cells, conflicts, revision links, citations, answerability, uncertainty, temporal constraints, dependency edges, and utility signals. Similarity search can be a useful access path, but it is not the memory model. The product-facing result is a `ContextPacket`, not an unstructured retrieval dump.

3. Model-assisted memory must stay proposal-only at the mutation boundary.

   A Steward model may propose draft cells, verification work, confidence changes, answerability labels, revision links, and frontier priority changes. It must not silently mutate committed truth. Deterministic policy validates or rejects proposals, and accepted decisions become auditable append-only database records.

4. Context collapse is a database problem before it is a prompt problem.

   Context collapse happens when an agent loses the reasons, uncertainty, version history, and operational constraints behind prior conclusions. ContinuityDB prevents this by preserving evidence and revision structure at write time, then checking out compact, task-relevant continuity slices instead of relying on long-context accumulation or lossy summaries.

5. Frontier memory quality can be tested without treating old benchmarks as current proof.

   A memory system should expose proof artifacts: checkout fixtures, lookup plans, commit manifests, replayable bundles, Steward evaluation suites, acceptance coverage, retained model responses, and quality-gate reports. ContinuityDB should make memory quality inspectable and regression-testable. Historical benchmark artifacts are useful retained records, but they are not current evidence unless refreshed against the active StateCell -> checkout(task,budget) -> ContextPacket contract.

## Innovation

ContinuityDB's innovation is not "a vector database for agents" or "a graph memory." The innovation is a database-native continuity model:

- `StateCell` as the primitive of operational truth.
- Append-only belief revision instead of in-place memory mutation.
- Evidence, citations, confidence, answerability, activation, utility, and temporal validity as first-class data.
- Native revision links for conflict, supersession, dependency, and audit relationships.
- Deterministic `checkout(task,budget)` that packs continuity state into `ContextPacket`s under task and token constraints.
- A proposal-only Steward layer for model-assisted database maintenance.
- Storage-kernel abstraction that keeps database semantics independent from any one storage backend.
- Retained artifact gates that make local model stewardship auditable before it is trusted.

This shifts LLM memory from application glue into a database contract.

## Expected Impact On LLM Memory

ContinuityDB should improve LLM memory and context by making every remembered fact operationally inspectable:

- Context becomes reproducible: the same query against the same commit can materialize the same continuity slice.
- Memory becomes revisable without erasure: newer evidence can supersede or conflict with older state while preserving the trail.
- Uncertainty remains visible: low-confidence or insufficient-evidence states can be carried into checkout instead of flattened into confident summaries.
- Citations survive compression: selected context can preserve source locators and audit traces.
- Frontier work is explicit: stale, high-impact, uncertain, or unresolved cells can stay visible as work items.
- Token budgets become database constraints: checkout can choose the most useful continuity slice instead of dumping all available memories into a prompt.

The expected result is not infinite memory. It is bounded, high-integrity continuity.

## How It Prevents Context Collapse

ContinuityDB prevents context collapse by refusing to store memory as only text.

When an LLM conversation, agent workflow, or external system produces useful state, ContinuityDB stores that state with the machinery needed to recover its meaning later: evidence, scope, time, confidence, dependencies, revision links, and commit identity. When an agent needs context, the database returns a continuity slice that contains the current state plus enough provenance and uncertainty to avoid hallucinated continuity.

This attacks the failure mode directly:

- Lost source evidence becomes cited audit traces.
- Lost version history becomes revision links and commit manifests.
- Lost uncertainty becomes confidence and answerability metadata.
- Lost task relevance becomes query and checkout constraints.
- Lost memory quality becomes replayable workload and Steward gate artifacts.
- Lost mutation accountability becomes proposal audits and deterministic policy decisions.

The frontier bet is that context windows will keep growing, but durable continuity will matter more than raw context length.

## Falsifiable Proof Obligations

ContinuityDB should earn its claims through evidence, not positioning. The roadmap should keep producing artifacts that prove or disprove the thesis:

- A checkout fixture can be generated, replayed, and compared deterministically.
- A file-backed store can preserve cells, commits, revision links, indexes, and corruption diagnostics across reopen and compaction.
- A checkout can materialize task-relevant context with citations, uncertainty, frontier metadata, and bounded token cost.
- A commit can be exported, imported, copied, and audited without losing revision relationships.
- A Steward proposal can be recorded, policy-evaluated, rejected without mutation, or accepted through a deterministic application path.
- A local model candidate can be evaluated against fixed Steward acceptance criteria with retained artifacts proving what was tested.
- A quality gate can reject incomplete, unstable, unsupported, or unauditable model memory behavior before it enters the trusted path.

If these proof obligations do not hold, the thesis is not proven.

## Non-Goals

ContinuityDB is not trying to become:

- A general agent runtime.
- A hidden autonomous memory mutator.
- A chat transcript store with better search.
- A vector database wrapper.
- A single-backend storage engine.
- A hosted-model-dependent memory service.

The database owns continuity semantics. Applications and agents use those semantics; they do not replace them.
