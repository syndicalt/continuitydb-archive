# Agent Behavior Failure Analysis

This report analyzes the live representative agent-behavior benchmark retained in
`target/representative-agent-behavior-benchmark-bundle-live/`. It is intentionally
critical. The current result does not justify a frontier claim by itself.

## Evidence Base

Primary artifacts:

- `target/representative-agent-behavior-benchmark-bundle-live/agent-behavior-execution-benchmark.json`
- `target/representative-agent-behavior-benchmark-bundle-live/agent-behavior-output-records.json`
- `target/representative-agent-behavior-benchmark-report-live.json`
- `target/representative-agent-behavior-benchmark-report-live.md`

The live run used `deepseek-v4-flash:cloud` through the local model runner. It
executed 32 task records across four strategies, with 24 scored records per
strategy in the retained execution benchmark.

Headline scored results:

| Strategy | Passed | Task success | Revision accuracy | Verification | Stale belief | Action regression |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `continuitydb_checkout` | 15 / 24 | 6250 bps | 2500 bps | 10000 bps | 0 bps | 0 bps |
| `vector_retrieval` | 8 / 24 | 3333 bps | 833 bps | 6666 bps | 0 bps | 0 bps |
| `raw_statecell` | 7 / 24 | 2916 bps | 0 bps | 6111 bps | 1111 bps | 0 bps |
| `transcript_summary` | 7 / 24 | 2916 bps | 833 bps | 5555 bps | 0 bps | 0 bps |

The checkout result has a wide confidence interval, 4313 to 8187 bps. That range
is too broad to support strong claims. It says the current system is probably
better than the weak baselines on this harness, but it does not say the
architecture is solved.

The most important pattern is not the aggregate score. `continuitydb_checkout`
went 3 / 3 on five task families and 0 / 3 on three task families:

| Task family | Checkout result |
| --- | ---: |
| `repo-ci-flake-triage` | 3 / 3 |
| `repo-commit-revert-risk` | 3 / 3 |
| `repo-dependency-upgrade-uncertainty` | 3 / 3 |
| `repo-issue-regression` | 3 / 3 |
| `repo-production-incident-followup` | 3 / 3 |
| `repo-docs-architecture-drift` | 0 / 3 |
| `repo-pr-review-correction` | 0 / 3 |
| `repo-release-asset-failure` | 0 / 3 |

That pattern is diagnostic. The system does not fail everywhere. It fails where
the answer must explicitly manage revised belief, invalidated context, and action
constraints. This is exactly the area where ContinuityDB is supposed to be
strong, so the failure matters.

## Bottom Line

The benchmark did not show that StateCell is useless. It showed something more
specific and more uncomfortable:

> Native continuity semantics do not automatically become useful model context.
> The checkout layer must compile state into an explicit task control packet at
> the right abstraction level.

Right now, ContinuityDB mostly stores and selects semantically relevant state.
That is not enough. The model still receives a prose packet and has to infer
which beliefs are current, which beliefs are invalidated, which actions are
forbidden, and which obligations must appear in the answer. Sometimes it infers
that correctly. Sometimes it collapses the packet into a generic recommendation.

This means the current architecture is not yet a breakthrough. It is a promising
storage and selection substrate with an underdeveloped context compiler.

## 1. Scorer False Negatives

Some of the 6250 bps result is measurement error.

The deterministic scorer currently treats revision as satisfied only when the
model output contains one of these strings:

`superseding`, `correction`, `stale`, `revised`, `failed`

This is too brittle. It rejects obviously acceptable variants such as
`superseded`, `supersede`, `revision`, and `revise`.

Concrete evidence:

- `repo-pr-review-correction`, trial 1: the output says, "Request revision" and
  "the older API approval is superseded by the newer review requirements." The
  scorer still marks `revision_accurate=false`.
- `repo-pr-review-correction`, trial 2: the output says, "Supersede the older
  API approval." The scorer still marks `revision_accurate=false`.
- `repo-docs-architecture-drift`, trial 2: the output says to "revise the
  documentation to reflect task-aware context packet compilation." The scorer
  still marks `revision_accurate=false`.

This means the absolute task-success score is not reliable. It undercounts
checkout and likely undercounts the baselines too.

However, this does not explain everything. `repo-release-asset-failure` produced
answers that were genuinely too weak: they said to verify the asset URL and
manifest, but often omitted the failed upload, stale success note, and forbidden
repeat action. That is not just a scorer problem.

Roadmap:

- Replace global keyword lists with per-task expected predicates.
- Record a reason for each predicate decision in the retained benchmark artifact.
- Expand deterministic lexical variants for obvious morphology:
  `supersede`, `superseded`, `superseding`, `revision`, `revise`, `revised`,
  `stale`, `invalidated`, `no longer valid`, `newer requirement`.
- Treat the deterministic scorer as a hard-negative screen, not the sole judge.
- Add blinded cross-model or human adjudication for disputed outputs.
- Report false-negative audits separately from model behavior.

Acceptance criteria:

- Every failed output has a retained predicate-level reason.
- Manual audit finds less than 10 percent obvious scorer false negatives.
- Benchmark reports distinguish `model_failed` from `scorer_uncertain`.

## 2. Packet Failures

The checkout packet is currently too prose-like. It contains useful facts, but it
does not expose a strong enough control surface.

Example checkout packet for `repo-release-asset-failure`:

> Current belief: release asset upload failed with HTTP 404 despite the stale
> success note. Evidence locator: github://release-upload-404. Do not repeat the
> failed upload command; verify release asset URL and manifest before retry.

The model outputs were:

- "Verify the release asset URL and manifest."
- "Verify the release asset URL and manifest before retrying the upload."
- "Verify the release asset URL and manifest to ensure correctness before any
  retry."

Those answers are not catastrophic, but they are not strong enough. A production
agent context provider should have forced the answer to carry the control facts:

- the previous success note is stale,
- the upload failed with HTTP 404,
- the same upload command must not be repeated,
- the URL and manifest must be verified before any retry.

The model retained only the verification step. The packet did not reliably
preserve the operational semantics under generation pressure.

This reveals the real architecture gap. We have a context packet, but not a
compiled task contract.

Roadmap:

- Replace single prose packets with typed checkout packets:
  - `current_truth`
  - `invalidated_beliefs`
  - `required_actions`
  - `forbidden_actions`
  - `required_verifications`
  - `uncertainty_constraints`
  - `evidence_to_cite`
  - `success_criteria`
- Compile prose only after the typed contract exists.
- Add a packet coverage check before model execution: every task requirement
  must appear in a visible packet field.
- Add packet-level tests that fail before any model call when a required control
  fact is missing.
- Track packet size by section, not only total token budget.

Acceptance criteria:

- The release-asset task packet has explicit `invalidated_beliefs`,
  `forbidden_actions`, and `required_verifications`.
- Each benchmark requirement traces to at least one packet field.
- Packet compilation failures are reported separately from model failures.

## 3. Model Compliance Failures

Some failures happen even when the packet contains the right words.

In `repo-release-asset-failure`, the packet explicitly says not to repeat the
failed upload command. The model did not repeat it, but it also did not preserve
that constraint in the answer. For an agent, omission can still be dangerous:
downstream execution may see only "verify before retry" and lose the stronger
"do not repeat the failed command" instruction.

This is different from retrieval failure. The context was present, but the model
did not transform it into a complete answer.

That means the benchmark currently mixes at least three concerns:

- Did retrieval select the right state?
- Did checkout compile the right packet?
- Did the model comply with the packet?

The current single `task_success` score hides that distinction.

Roadmap:

- Require structured model outputs during benchmark execution:
  - `decision`
  - `current_truth_used`
  - `invalidated_context`
  - `do_not_do`
  - `required_verification`
  - `uncertainty_or_confidence`
  - `next_action`
- Add a compliance validator after model generation.
- Permit one repair pass only when the model omits a required slot already
  present in the packet.
- Report compliance repair rate as a first-class metric.
- Keep natural-language rendering separate from benchmark scoring.

Acceptance criteria:

- Model outputs can no longer pass by giving a generic recommendation.
- Missing required slots produce `model_compliance_failure`, not generic task
  failure.
- Checkout can be evaluated without confounding it with generation compliance.

## 4. Primitive Failures

The current StateCell shape may be semantically rich but operationally incomplete.
It is good at representing beliefs, evidence, confidence, and lifecycle metadata.
It is weaker at representing context as an executable control contract.

The failed task families point to missing first-class concepts:

- invalidated belief,
- superseding belief,
- forbidden action,
- required action,
- required verification,
- uncertainty obligation,
- applicability condition,
- invalidation condition,
- response contract,
- checkout contract.

These should not be only prose fields. If they matter to agent behavior, they
need to be queryable, composable, and testable.

This does not mean ContinuityDB should become a workflow engine. It means the
database primitive must distinguish epistemic state from operational obligation.
An agent does not merely ask, "what facts are relevant?" It asks, "given the
current state of the world, what must I believe, avoid, verify, and do next?"

Roadmap:

- Define StateCell v2 around two layers:
  - epistemic layer: belief, evidence, uncertainty, revision, validity,
    provenance;
  - control layer: required action, forbidden action, verification obligation,
    applicability, invalidation, response contract.
- Make checkout compile from both layers into a typed task-control packet.
- Add traceability from packet fields back to StateCell IDs and evidence
  locators.
- Add contract coverage tests: every benchmark requirement must be represented
  in StateCell fields, selected by checkout, and emitted in the packet.
- Keep routing simple. The goal is not more modes. The goal is one pipeline with
  explicit stages and observable failure points.

Acceptance criteria:

- StateCell v2 can represent "the prior success belief is invalidated by this
  failed upload evidence" without relying on prose interpretation.
- Checkout emits `forbidden_actions` and `required_verifications` directly from
  structured state.
- The benchmark can attribute a failure to storage, selection, packet
  compilation, or model compliance.

## Recommended Pipeline

The product should converge to one pipeline:

1. Agent receives a task.
2. Task is parsed into intent, required decision type, risk, and constraints.
3. ContinuityDB retrieves candidate StateCells by topic, lifecycle, evidence,
   revision, validity, and task intent.
4. Checkout resolves conflicts and compiles a typed task-control packet.
5. The model receives the packet and must answer in required slots.
6. A validator checks whether required packet obligations survived generation.
7. The final answer or action is emitted with evidence and uncertainty attached.
8. New evidence, outcomes, failures, and revisions are written back as StateCells.

This is still a database if the durable product is the state substrate plus
checkout semantics. It stops being a database if the core value becomes a pile of
task-specific prompt routes. The design pressure should therefore be:

> More structure in the primitive, fewer special checkout modes.

## Roadmap

### Sprint 1: Measurement Repair

Fix the benchmark before drawing more conclusions.

Deliverables:

- predicate-level scorer reasons,
- expanded deterministic variants,
- per-task expected predicates,
- false-negative audit report,
- separate `scorer_uncertain`, `model_failed`, and `packet_failed` outcomes.

Do not run bigger benchmarks until this is done. Larger runs with bad scoring
only produce more misleading numbers.

### Sprint 2: Typed Checkout Packet

Replace prose-only checkout output with a structured packet contract.

Deliverables:

- typed packet schema,
- packet compiler,
- packet coverage validator,
- retained packet artifact,
- tests for the three failed task families.

The goal is not to add another checkout mode. The goal is to make the one
checkout path inspectable and contract-bearing.

### Sprint 3: Structured Model Compliance

Stop asking the model for an unconstrained paragraph during evaluation.

Deliverables:

- structured response schema,
- compliance validator,
- one-pass repair protocol,
- compliance failure metrics,
- natural-language renderer that is downstream of the scored structure.

This will tell us whether ContinuityDB failed or whether the model failed to use
valid context.

### Sprint 4: StateCell v2 Control Semantics

Promote operational obligations into first-class StateCell semantics.

Deliverables:

- structured invalidation and supersession fields,
- structured required and forbidden action fields,
- verification obligation field,
- applicability and invalidation conditions,
- traceable evidence links for every control obligation.

This is the architectural fork. If StateCell cannot carry these concepts cleanly,
then the primitive is not yet the right one for lifecycle context.

### Sprint 5: Re-run the Representative Benchmark

Only after Sprints 1 through 3, re-run the same live benchmark.

Required reporting:

- retrieval success,
- packet coverage,
- model compliance,
- final task success,
- repair rate,
- latency split between checkout and model inference,
- confidence intervals,
- retained outputs and adjudication records.

Success should not be "checkout is best among weak baselines." A credible next
bar is:

- checkout task success above 8000 bps,
- revision accuracy above 7500 bps,
- no stale-belief regressions,
- no repeated failed actions,
- manual false-negative rate below 10 percent,
- failure attribution available for every miss.

## What This Means For The Thesis

The original broad thesis, "StateCell wins because it stores better facts," is
not strong enough and may be wrong.

The stronger thesis is:

> Agent context requires a durable epistemic and operational state primitive
> whose checkout operation compiles task-specific control packets: current truth,
> invalidated beliefs, uncertainty, evidence, required verification, and
> forbidden actions.

That thesis is still plausible. The current implementation does not prove it.

The benchmark result should therefore be treated as a warning, not a failure of
the entire idea. It tells us exactly where the architecture is underpowered:
continuity semantics must survive the transition from database state to model
instruction. If they do not, the system becomes a more complicated way to do
retrieval.

## Immediate Decision

Do not optimize the existing prose packet further as the main path. That risks
polishing noise.

Do not run 100k or 1M live benchmarks yet. Scale does not answer the current
question.

Do build the typed packet and attribution layer. If that does not materially
improve the failed task families, then we should seriously reconsider whether
StateCell is the right primitive or whether ContinuityDB should be reduced to a
smaller library inside another memory architecture.
