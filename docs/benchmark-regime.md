# Benchmark Regime

ContinuityDB benchmarks should measure the product claim:

```text
Does checkout(task, budget) improve agent behavior over time?
```

They should not measure whether ContinuityDB wins an internal semantic rubric.

## Phase 1: Small Objective Harness

The first harness is `agent-behavior-benchmark`.

```bash
cargo run -p continuitydb-cli -- agent-behavior-benchmark \
  --report-path target/agent-behavior-benchmark.json
```

It compares four strategies over 10 deterministic tasks with 5 turns each:

- `transcript_summary`
- `vector_retrieval`
- `raw_statecell`
- `continuitydb_checkout`

The metrics are downstream behavior metrics:

- `task_success_rate_bps`
- `stale_belief_rate_bps`
- `revision_accuracy_bps`
- `unsupported_certainty_rate_bps`
- `verification_rate_bps`
- `context_budget_fit_bps`
- `action_regression_rate_bps`

This phase is intentionally deterministic and simulated. It exists to pin the benchmark contract and make the thesis falsifiable before expensive model runs.

## Phase 2: Same-Model Task Execution

Replace simulated outcomes with actual same-model task execution:

- same task prompts
- same context budget
- same tool availability
- same model and decoding settings
- repeated seeds or repeated runs

Objective checks should lead: stale action repeated, correction used, uncertainty hedged, evidence checked, tests passed, final task succeeded.

Phase 2 starts with a canonical task matrix rather than hand-written prompts:

```bash
cargo run -p continuitydb-cli -- agent-behavior-task-matrix \
  --tasks-path target/agent-behavior-tasks.json \
  --report-path target/agent-behavior-task-matrix.json
```

The matrix currently emits 10 task scenarios across 4 context strategies:

- `transcript_summary`
- `vector_retrieval`
- `raw_statecell`
- `continuitydb_checkout`

That yields 40 runner-ready task requests. Each scenario keeps the same prompt and objective requirements across strategies while changing only the context packet.

The scorer can also consume retained answer records directly. Any local model runner, cloud model runner, or scripted smoke runner can produce the same JSON record shape:

```json
[
  {
    "task_id": "release-upload",
    "strategy": "continuitydb_checkout",
    "prompt": "The GitHub release upload failed with HTTP 404. What next?",
    "context_packet": "Superseding correction: release asset upload failed with HTTP 404; verify the release asset URL before retrying.",
    "model_output": "Do not repeat the failed upload command. Use the superseding correction, verify the release asset URL, and treat the prior success belief as stale.",
    "requirements": {
      "requires_revision": true,
      "requires_uncertainty": false,
      "requires_verification": true,
      "has_stale_trap": true,
      "has_known_failed_action": true
    }
  }
]
```

Score retained records with:

```bash
cargo run -p continuitydb-cli -- score-agent-behavior-outputs \
  --records-path target/agent-behavior-output-records.json \
  --report-path target/agent-behavior-execution-benchmark.json
```

The scorer preserves raw prompts, context packets, model outputs, requirements, and deterministic outcome labels. This makes a failed or surprising run auditable before the benchmark grows into an expensive repeated-run matrix.

To run tasks through an actual model adapter and score them in one step, use:

```bash
cargo run -p continuitydb-cli -- run-agent-behavior-outputs \
  --tasks-path target/agent-behavior-tasks.json \
  --runner "$(command -v node)" \
  --runner-arg=scripts/agent_behavior_model_runner.mjs \
  --runner-arg=--model \
  --runner-arg=deepseek-v4-flash:cloud \
  --trials 3 \
  --records-path target/agent-behavior-output-records.json \
  --report-path target/agent-behavior-execution-benchmark.json
```

For a CI-safe dry run, replace the model arguments with:

```bash
--runner-arg=scripts/agent_behavior_model_runner.mjs --runner-arg=--dry-run
```

Use `--runner-arg=--flag` form for runner arguments that begin with `--`, so the ContinuityDB CLI does not parse them as its own flags.

`run-agent-behavior-outputs` invokes the runner once per task per trial. The runner receives one JSON object on stdin:

```json
{
  "format": "continuitydb.agent_behavior_runner_input",
  "format_version": 1,
  "task_id": "release-upload",
  "trial_index": 0,
  "strategy": "continuitydb_checkout",
  "prompt": "The GitHub release upload failed with HTTP 404. What next?",
  "context_packet": "Superseding correction: release asset upload failed with HTTP 404; verify the release asset URL before retrying.",
  "requirements": {
    "requires_revision": true,
    "requires_uncertainty": false,
    "requires_verification": true,
    "has_stale_trap": true,
    "has_known_failed_action": true
  }
}
```

The runner must return JSON on stdout:

```json
{
  "model_output": "Do not repeat the failed upload command. Use the superseding correction, verify the release asset URL, and treat the prior success belief as stale."
}
```

The retained records include `model_latency_ms` when they are produced by the CLI runner. The scored report includes:

- `trial_count`
- `strategy_stability`, with per-strategy mean/min/max task-success rates across trials
- `task_success_confidence_interval_bps`, an approximate 95% confidence interval for every strategy
- `strategy_latency`, with p50/p95/p99 model-runner latency per strategy

This keeps provider-specific execution outside the metric contract while still making the benchmark a real repeated model-execution benchmark.

To package a reproducible benchmark artifact bundle in one command, use:

```bash
cargo run -p continuitydb-cli -- agent-behavior-benchmark-bundle \
  --artifact-dir target/agent-behavior-benchmark-bundle \
  --runner "$(command -v node)" \
  --runner-arg=scripts/agent_behavior_model_runner.mjs \
  --runner-arg=--model \
  --runner-arg=deepseek-v4-flash:cloud \
  --runner-arg=--max-tokens \
  --runner-arg=1024 \
  --trials 3
```

The bundle command writes:

- `agent-behavior-benchmark.manifest.json`
- `agent-behavior-task-matrix.json`
- `agent-behavior-tasks.json`
- `agent-behavior-output-records.json`
- `agent-behavior-execution-benchmark.json`

The manifest records the runner, runner arguments, trial count, task count, execution-record count, byte sizes, and stable file fingerprints. Use the bundle output as the retained artifact for reports, paper drafts, and release notes. For a dry run, replace the model arguments with `--runner-arg=--dry-run`.

Interpret a retained bundle with:

```bash
cargo run -p continuitydb-cli -- agent-behavior-benchmark-report \
  --artifact-dir target/agent-behavior-benchmark-bundle \
  --report-path target/agent-behavior-benchmark-report.json \
  --summary-path target/agent-behavior-benchmark-report.md
```

The interpreted report classifies the evidence tier, checks marketability gates, and extracts the headline checkout metrics. A dry-run bundle is classified as artifact validation only. A representative live model bundle becomes marketable only when it uses the representative corpus, retains live model outputs, runs at least 3 trials, includes confidence intervals, includes latency percentiles, and satisfies the thesis-signal gates:

- checkout is best or tied on task success
- checkout is lowest or tied on stale-belief rate
- checkout is lowest or tied on action-regression rate
- checkout task success is at least 5000 bps

Use `--require-marketable` in CI or release checks when the artifact is intended to support external claims:

```bash
cargo run -p continuitydb-cli -- agent-behavior-benchmark-report \
  --artifact-dir target/representative-agent-behavior-benchmark-bundle \
  --report-path target/representative-agent-behavior-benchmark-report.json \
  --summary-path target/representative-agent-behavior-benchmark-report.md \
  --require-marketable
```

The command still writes the report before exiting nonzero, so failed gates leave an auditable artifact explaining what is missing.

## Phase 3: Representative Corpus

After the small curated corpus is stable, switch the same runner/scorer protocol to representative repo-lifecycle tasks:

```bash
cargo run -p continuitydb-cli -- agent-behavior-task-matrix \
  --representative-corpus \
  --tasks-path target/representative-agent-behavior-tasks.json \
  --report-path target/representative-agent-behavior-task-matrix.json
```

To retain the full representative bundle:

```bash
cargo run -p continuitydb-cli -- agent-behavior-benchmark-bundle \
  --representative-corpus \
  --artifact-dir target/representative-agent-behavior-benchmark-bundle \
  --runner "$(command -v node)" \
  --runner-arg=scripts/agent_behavior_model_runner.mjs \
  --runner-arg=--model \
  --runner-arg=deepseek-v4-flash:cloud \
  --runner-arg=--max-tokens \
  --runner-arg=1024 \
  --trials 3
```

Reasoning models may consume much of `num_predict` before producing the final answer. If a live run fails with `agent behavior runner output missing model_output`, increase `--max-tokens` and rerun; the CLI error includes the failing task id, strategy, and trial.

Then generate the interpreted report:

```bash
cargo run -p continuitydb-cli -- agent-behavior-benchmark-report \
  --artifact-dir target/representative-agent-behavior-benchmark-bundle \
  --report-path target/representative-agent-behavior-benchmark-report.json \
  --summary-path target/representative-agent-behavior-benchmark-report.md
```

The current representative matrix keeps the same four strategies and covers repo-lifecycle cases from:

- issues
- pull requests
- commits
- docs
- CI failures
- release failures
- invalidated assumptions
- known failed actions
- superseding corrections

The benchmark should report confidence intervals and p50/p95/p99 runner latency before external claims. Representative dry runs are useful only for artifact validation; marketable evidence requires live model outputs retained in the bundle.

## Phase 4: Thesis Falsification

The thesis-falsification benchmark asks a narrower question than the old rubric:

```text
Does durable epistemic/control state improve long-horizon agent behavior beyond strong hybrid memory retrieval?
```

Run it with:

```bash
cargo run -p continuitydb-cli -- thesis-falsification-benchmark \
  --report-path target/thesis-falsification-benchmark.json \
  --summary-path target/thesis-falsification-benchmark.md
```

The corpus is intentionally small and adversarial: 24 retained documents feeding
8 lifecycle/control tasks. The benchmark compares:

- `transcript_summary`
- `vector_retrieval`
- `gbrain_hybrid_memory`
- `continuitydb_control_packet`

`gbrain_hybrid_memory` is a deterministic local adapter shaped after GBrain's
published architecture: hybrid retrieval, graph-linked synthesis, citations, and
gap notes. It derives answers from the retained context packet, but it is not a
live GBrain execution and must not be reported as one. A live GBrain adapter
remains a separate external-integration milestone because the current local
environment does not ship with Bun/GBrain installed, and the benchmark contract
should stabilize before external setup variance enters the result.

The benchmark uses predicate-level scoring rather than a global keyword rubric:

- correct decision
- stale belief rejection
- revision preservation
- forbidden-action avoidance
- verification trigger
- uncertainty faithfulness
- evidence grounding
- helpfulness

The judgement is intentionally decisive:

- if the strong GBrain-style baseline performs well without StateCell, the
  original "specialized database primitive is necessary" thesis collapses;
- if ContinuityDB beats the strong baseline by at least 2000 bps on task
  success, beats it on revision, forbidden-action, and uncertainty predicates,
  and ties or beats it on verification, the narrower durable
  epistemic/control-layer thesis survives;
- if ContinuityDB does not beat the strong baseline on those predicates, the
  thesis collapses for this corpus.

## Phase 5: Marketable Evidence

Only after phases 1-4 should results be used externally. Marketable claims require:

- objective downstream task metrics
- repeated runs
- confidence intervals
- p50/p95/p99 latency percentiles
- public corpus or reproducible generator
- raw context packets retained
- evaluator outputs retained
- clear limitations and failure cases

The claim to prove is narrow and concrete:

```text
ContinuityDB checkout reduces stale-belief and repeated-failure behavior
without harming task success or exceeding the context budget.
```
