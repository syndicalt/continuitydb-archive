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

## Steward Milestones

1. Define `StewardProposal` types without invoking any model. Implemented in `continuitydb-steward`.
2. Add policy validation for accepting and rejecting proposals. Implemented in `continuitydb-steward`.
3. Persist accepted and rejected proposals for audit. Implemented as an in-memory append-only ledger in `continuitydb-steward`; storage-backed persistence remains future work.
4. Build a deterministic mock steward for test-first development. Implemented in `continuitydb-steward`.
5. Add local model inference behind a feature flag. Implemented as a `local-model` backend boundary in `continuitydb-steward`; concrete llama.cpp/mistral.rs runners remain future work.
6. Evaluate small open-source steward models against fixed proposal-quality tests. Implemented as a `local-model` evaluation harness with candidate metadata and deterministic pass/fail reasons; real runner benchmarking remains future work.
7. Add frontier/watch integration so the Steward can propose refresh and verification work. Implemented as deterministic frontier watch events that emit `RequestVerification` and `MarkFrontier` proposals in `continuitydb-steward`; durable subscriptions remain future work.

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
- Explicit uncertainty when evidence is insufficient.
- Deterministic policy rejection of invalid proposals.

## Research Sources

- Qwen2.5-0.5B-Instruct model card: <https://huggingface.co/Qwen/Qwen2.5-0.5B-Instruct>
- Qwen3-0.6B model card: <https://huggingface.co/Qwen/Qwen3-0.6B>
- SmolLM2-360M-Instruct model files/card: <https://huggingface.co/HuggingFaceTB/SmolLM2-360M-Instruct>
- llama.cpp GGUF and grammar-constrained inference: <https://github.com/ggml-org/llama.cpp>
- mistral.rs Rust inference engine: <https://docs.rs/crate/mistralrs/latest>
