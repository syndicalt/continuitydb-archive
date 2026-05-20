# ContinuityDB Roadmap

This roadmap tracks frontier items that extend ContinuityDB beyond the initial deterministic Rust foundation.

## Roadmap Principles

- Preserve ContinuityDB as a datastore, not an agent runtime.
- Keep deterministic database semantics at the commit boundary.
- Treat model outputs as proposals, not hidden truth mutations.
- Require every accepted model-assisted change to be auditable as evidence.
- Prefer small, embeddable, open-source models before hosted model dependencies.

## Frontier Item: Embedded Database Steward

ContinuityDB should eventually include an embedded agentic model layer that acts as a steward for the database itself. This is not a general agent orchestrator and must not run application workflows. Its job is to maintain the epistemic health of the datastore.

The Steward may propose:

- StateCell creation from new evidence.
- Supersession and conflict links.
- Confidence adjustments.
- Answerability labels.
- Frontier priority changes.
- Checkout summaries and uncertainty explanations.
- Verification or refresh tasks for stale or high-impact cells.

The Steward must not directly mutate committed truth. It emits structured proposals. A deterministic policy layer validates, rejects, or accepts those proposals. Accepted proposals become append-only StateCells or revision links with provenance showing the source evidence, model identity, prompt/profile, policy decision, and resulting database mutation.

Boundary rule:

> ContinuityDB may use an embedded model to propose what state needs attention; deterministic database policy decides what becomes committed truth.

## Steward Architecture Track

```text
External apps and agents
  ingest evidence, request checkout, provide feedback

ContinuityDB API
  deterministic request boundary

Steward model layer
  proposes revisions, conflicts, answerability, summaries, frontier work

Policy and validation layer
  validates proposal schema, confidence, permissions, and audit requirements

Semantic engine
  StateCells, evidence, bitemporality, revision, checkout, audit

Storage kernel
  append log, graph, temporal, text, vector, payload storage
```

## Storage Kernel Milestones

1. Define the minimal `StorageKernel` append and lookup contract. Implemented in `continuitydb-kernel`.
2. Provide an in-memory correctness kernel for deterministic tests. Implemented in `continuitydb-memory`.
3. Add the first durable embedded kernel. Implemented as an append-only JSONL `FileKernel` in `continuitydb-kernel` with nested database directory creation; indexed production storage remains future work.
4. Add first-class StateCell activation filtering. Implemented in `CellLookup` across memory and file kernels.
5. Add first-class StateCell answerability filtering. Implemented in `CellLookup` across memory and file kernels.
6. Add first-class evidence-source and minimum-confidence filtering. Implemented in `CellLookup` across memory and file kernels.

## Checkout Milestones

1. Push deterministic checkout constraints into storage lookup. Implemented for scope, valid time, answerability question, evidence source, and minimum confidence in `continuitydb-checkout`.
2. Add selected-cell metadata to checkout slices. Implemented audit traces, uncertainty entries, and frontier recommendations in `continuitydb-checkout`.
3. Add deterministic checkout alternatives. Implemented token-budget omission metadata with reason, citations, and confidence in `continuitydb-checkout`.
4. Rank checkout candidates by deterministic utility-aware score. Implemented by combining max evidence confidence with `StateCell` utility feedback before token-budget packing in `continuitydb-checkout`.

## Utility Feedback Milestones

1. Add first-class StateCell utility feedback primitives. Implemented as bounded relevance, recency, and decision-impact scores in `continuitydb-core`, with neutral defaults for new and previously serialized cells.

## CLI Milestones

1. Expose deterministic checkout JSON from the CLI. Implemented as `continuitydb demo-checkout`, showing selected cells, audit traces, uncertainty, frontier recommendations, and alternatives.

## Steward Milestones

1. Define `StewardProposal` types without invoking any model. Implemented in `continuitydb-steward`.
2. Add policy validation for accepting and rejecting proposals. Implemented in `continuitydb-steward`.
3. Persist accepted and rejected proposals for audit. Implemented as an in-memory append-only ledger, a pluggable `ProposalLedgerStore` contract, a JSONL `FileProposalStore`, and a generic `StorageKernel`-backed proposal audit adapter in `continuitydb-steward`; specialized production-engine adapters remain future work.
4. Build a deterministic mock steward for test-first development. Implemented in `continuitydb-steward`.
5. Add local model inference behind a feature flag. Implemented as a `local-model` backend boundary, local executable runner, and deterministic llama.cpp/mistral.rs runner profiles in `continuitydb-steward`; real runtime execution remains future work.
6. Evaluate small open-source steward models against fixed proposal-quality tests. Implemented as a `local-model` evaluation harness with candidate metadata, deterministic pass/fail reasons, an executable runner benchmark fixture, durable JSONL benchmark baseline records, a recorder API for configured real-runtime baseline collection, latest-baseline lookup for candidate regression gates, deterministic baseline regression comparison, and a record-and-compare gate report; environment-specific real model baseline artifacts remain future work.
7. Add frontier/watch integration so the Steward can propose refresh and verification work. Implemented as deterministic frontier watch events that emit `RequestVerification` and `MarkFrontier` proposals, durable frontier subscription records with in-memory and JSONL file-backed stores, and a subscription runner that filters incoming watch events through stored subscriptions in `continuitydb-steward`.

## Small Embeddable Model Track

The smallest feasible first candidate is **Qwen2.5-0.5B-Instruct**. It is an Apache-2.0 instruction-tuned model with about 0.49B parameters, long context, and explicit model-card claims around instruction following and structured JSON generation. Those traits matter more for a database steward than general chat quality because the Steward must emit constrained proposal objects.

Evaluation candidates:

- **Default feasibility candidate:** `Qwen/Qwen2.5-0.5B-Instruct`
  - Why: smallest current candidate that is explicitly instruction-tuned and claims improved structured output behavior.
  - Use for: first real Steward proposal experiments.
- **Current reasoning candidate:** `Qwen/Qwen3-0.6B`
  - Why: slightly larger, newer Qwen line with configurable thinking behavior.
  - Use for: compare proposal quality against Qwen2.5-0.5B.
- **Ultra-small experimental candidate:** `HuggingFaceTB/SmolLM2-360M-Instruct`
  - Why: smaller on-device instruction model.
  - Use for: measure the lower bound; do not assume it is reliable enough for default stewardship.
- **Smoke-test-only candidate:** `HuggingFaceTB/SmolLM2-135M-Instruct`
  - Why: extremely small, useful for CI or toy constrained-output tests if it can follow the schema.
  - Use for: optional experiments, not default stewardship.

Recommended inference path:

- Prefer a local feature-gated backend.
- Evaluate `llama.cpp`/GGUF first for portability and grammar-constrained JSON output.
- Evaluate `mistral.rs` as the Rust-native integration path for GGUF and future in-process inference.
- Keep hosted model APIs outside the core engine.

## Steward Acceptance Tests

Before adding a real model dependency, create fixed test corpora and score proposals for:

- Valid JSON/schema conformance.
- Correct conflict versus supersession classification.
- Evidence citation preservation.
- No unsupported claims beyond source evidence.
- Stable output under low temperature.
- Explicit uncertainty when evidence is insufficient; scoreable through required rationale terms.
- Deterministic policy rejection of invalid proposals.

## Research Sources

- Qwen2.5-0.5B-Instruct model card: <https://huggingface.co/Qwen/Qwen2.5-0.5B-Instruct>
- Qwen3-0.6B model card: <https://huggingface.co/Qwen/Qwen3-0.6B>
- SmolLM2-360M-Instruct model files/card: <https://huggingface.co/HuggingFaceTB/SmolLM2-360M-Instruct>
- llama.cpp GGUF and grammar-constrained inference: <https://github.com/ggml-org/llama.cpp>
- mistral.rs Rust inference engine: <https://docs.rs/crate/mistralrs/latest>
