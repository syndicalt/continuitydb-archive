# ContinuityDB Roadmap

> Status: closed.
>
> This roadmap is historical context, not current execution guidance. The
> original ContinuityDB product thesis has been marked failed after
> falsification work showed that ordinary hybrid memory and retrieval systems
> can recover much of the useful evidence without a specialized StateCell
> datastore. Do not continue this roadmap as product work. See
> [Project Postmortem](project-postmortem.md).

ContinuityDB is being refactored around one product shape:

```text
StateCell -> checkout(task, budget) -> ContextPacket
```

## Principles

- Keep the public model small.
- Make checkout the product primitive.
- Preserve evidence, uncertainty, revision, lifecycle, and dependency semantics inside packets.
- Treat raw/static rendering and model assistance as diagnostics or implementation controls.
- Remove benchmark artifacts that do not measure real agent behavior.

## Near-Term Work

1. Simplify checkout naming and defaults.
   - Default checkout compilation is `Automatic`.
   - Static rendering is `RawBaseline` and should be used only for diagnostics and ablations.
   - Model-shaped packets are `ModelAssisted` and must remain validated and evidence-backed.

2. Reduce public API leakage.
   - Keep `checkout(task,budget)` as the primary mental model.
   - Avoid making compiler internals part of normal product documentation.
   - Keep packet-plan metadata auditable for debugging and tests.

3. Harden packet contracts.
   - Lifecycle `DoNotUseForAnswer`, invalidation, uncertainty, and evidence requirements must survive token pressure.
   - Trajectory reuse must require applicability and must respect invalidation.
   - Packet validation should reject context that looks complete but lost its semantic contract.

4. Replace historical benchmarks.
   - Old vector, graph, live-provider, adversarial-rubric, and long-context artifacts are not current evidence.
   - The first replacement is `agent-behavior-benchmark`, which pins downstream behavior metrics over 10 deterministic tasks.
   - Future evaluation should measure downstream agent behavior: revision accuracy, hedging quality, stale-belief avoidance, hallucinated-certainty rate, and long-horizon task success.
   - Keep storage/workload diagnostics for engineering regressions, but do not present them as thesis proof.

5. Keep storage boring and embeddable.
   - Maintain memory and file-backed kernels.
   - Continue toward a production storage engine only where it improves durability, queryability, and checkout performance.

## Current Definition Of Done

ContinuityDB is on track when a caller can understand the system without learning benchmark modes:

```text
append StateCells
call checkout with a task and token budget
receive a ContextPacket with evidence-backed operational context
append new evidence after the agent acts
```
