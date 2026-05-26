import assert from "node:assert/strict";
import test from "node:test";

import {
  answerPrompt,
  buildCells,
  eventForTurn,
  generateAnswer,
  mapWithConcurrency,
  parseJudgeJson,
  retrieve,
  runJudge,
  summarize,
} from "./long_context_experiment.mjs";

test("runJudge supports deterministic judging during live answer runs", async () => {
  let ollamaCalled = false;
  const result = await runJudge({
    dryRun: false,
    judgeModel: "deterministic",
    context: [
      {
        id: "cell-release-1-1",
        release: "release-1",
        kind: "surprise",
        text: "release-1: GitHub Release asset upload returned HTTP 404 after the earlier success claim.",
        confidence: 0.05,
        impact: 1,
        tokenCost: 22,
        currentTruth: "blocked",
        supersedes: "cell-release-1-0",
        conflicts: "cell-release-1-0",
        uncertainty: 0.286,
        surpriseBits: 4.322,
        citation: "workflow://release-1/4",
      },
    ],
    event: {
      truth: "blocked",
      expected: "Do not treat the release as shipped after the 404.",
      prompt: "Can we still treat release-1 as shipped after the 404?",
    },
    answer: "Current answer: not shipped after the 404; rerun or verify before acting. Evidence: workflow://release-1/4.",
    ollama: async () => {
      ollamaCalled = true;
      throw new Error("deterministic judge should not call Ollama");
    },
    ollamaUrl: "http://127.0.0.1:11434",
    timeoutMs: 1,
  });

  assert.equal(ollamaCalled, false);
  assert.equal(result.model, "deterministic");
  assert.equal(result.ok, true);
  assert.equal(result.score.rationale, "deterministic rubric judge");
  assert.equal(result.error, null);
  assert.ok(result.score.overall > 0);
});

test("parseJudgeJson accepts fenced JSON judge responses", () => {
  const parsed = parseJudgeJson(`\`\`\`json
{
  "revision_accuracy": 90,
  "stale_belief_avoidance": 95,
  "uncertainty_calibration": 80,
  "hedging_quality": 70,
  "surprise_scavenging": 0,
  "evidence_grounding": 100,
  "hallucinated_certainty": 95,
  "overall": 85,
  "rationale": "valid fenced response"
}
\`\`\``);

  assert.equal(parsed.overall, 85);
  assert.equal(parsed.rationale, "valid fenced response");
});

test("parseJudgeJson salvages complete numeric scores from truncated fenced responses", () => {
  const parsed = parseJudgeJson(`\`\`\`json
{
  "revision_accuracy": 90,
  "stale_belief_avoidance": 95,
  "uncertainty_calibration": 80,
  "hedging_quality": 70,
  "surprise_scavenging": 0,
  "evidence_grounding": 100,
  "hallucinated_certainty": 95,
  "overall": 85,
  "rationale": "The judge response was truncated`);

  assert.equal(parsed.overall, 85);
  assert.equal(parsed.revision_accuracy, 90);
  assert.equal(parsed.rationale, "");
});

test("mapWithConcurrency preserves order while running bounded concurrent work", async () => {
  let active = 0;
  let peak = 0;
  const results = await mapWithConcurrency([30, 20, 10, 0], 2, async (delay, index) => {
    active += 1;
    peak = Math.max(peak, active);
    await new Promise((resolve) => setTimeout(resolve, delay));
    active -= 1;
    return index;
  });

  assert.deepEqual(results, [0, 1, 2, 3]);
  assert.equal(peak, 2);
});

test("adversarial scenario makes summary lose the conflict trail under tight context", () => {
  const events = Array.from({ length: 7 }, (_, index) => eventForTurn(index + 1, 1, "adversarial"));
  const question = events[6];
  const cells = buildCells(events.slice(0, 7));

  const continuity = retrieve("continuitydb", cells, question, 45);
  const summary = retrieve("summary", cells, question, 45);

  assert.equal(question.kind, "question");
  assert.match(question.expected, /ambiguous/i);
  assert.equal(summary.length, 1);
  assert.equal(summary.some((cell) => cell.conflicts), false);
  assert.equal(continuity.some((cell) => cell.conflicts), true);
  assert.equal(continuity.some((cell) => cell.text.includes("ambiguous CDN cache signal")), true);
});

test("continuity checkout keeps latest verified frontier before older conflict trail", () => {
  const events = Array.from({ length: 11 }, (_, index) => eventForTurn(index + 1, 1, "adversarial"));
  const question = events[10];
  const cells = buildCells(events.slice(0, 11));

  const continuity = retrieve("continuitydb", cells, question, 45);

  assert.equal(question.kind, "question");
  assert.match(question.expected, /rerun/i);
  assert.equal(continuity[0].kind, "revision");
  assert.equal(continuity.some((cell) => cell.text.includes("HTTP 404")), true);
});

test("continuity checkout does not let stale duplicate evidence displace verified frontier", () => {
  const events = Array.from({ length: 13 }, (_, index) => eventForTurn(index + 1, 1, "adversarial"));
  const question = events[12];
  const cells = buildCells(events.slice(0, 13));

  const continuity = retrieve("continuitydb", cells, question, 45);

  assert.equal(question.kind, "question");
  assert.match(question.expected, /duplicated 404/i);
  assert.equal(continuity[0].kind, "revision");
  assert.equal(continuity.some((cell) => cell.text.includes("duplicated incident")), false);
});

test("continuity checkout preserves current belief, decisive contradiction, and active risk under tight budget", () => {
  const events = Array.from({ length: 16 }, (_, index) => eventForTurn(index + 1, 1, "adversarial"));
  const question = events[15];
  const cells = buildCells(events.slice(0, 16));

  const continuity = retrieve("continuitydb", cells, question, 45);
  const combined = continuity.map((cell) => cell.text).join(" ");

  assert.equal(question.kind, "question");
  assert.match(question.expected, /checksum provenance/i);
  assert.match(combined, /asset-present after rerun/i);
  assert.match(combined, /HTTP 404/i);
  assert.match(combined, /checksum provenance/i);
});

test("compact continuity packet is answer-friendly and citation anchored under tight budget", () => {
  const events = Array.from({ length: 16 }, (_, index) => eventForTurn(index + 1, 1, "adversarial"));
  const question = events[15];
  const cells = buildCells(events.slice(0, 16));

  const continuity = retrieve("continuitydb", cells, question, 45);
  const combined = continuity.map((cell) => `${cell.text} ${cell.citation}`).join(" ");
  const cost = continuity.reduce((sum, cell) => sum + cell.tokenCost, 0);

  assert.equal(question.kind, "question");
  assert.ok(cost <= 45);
  assert.match(combined, /Current belief: asset-present after rerun/i);
  assert.match(combined, /Decisive contradiction: HTTP 404 invalidated first success/i);
  assert.match(combined, /Active risk: checksum provenance unknown/i);
  assert.match(combined, /Safest action: verify provenance/i);
  assert.match(combined, /workflow:\/\/release-1-1\/10/);
  assert.match(combined, /workflow:\/\/release-1-1\/4/);
  assert.match(combined, /workflow:\/\/release-1-1\/14/);
});

test("continuity answer prompt treats role packet as authoritative checkout view", () => {
  const events = Array.from({ length: 16 }, (_, index) => eventForTurn(index + 1, 1, "adversarial"));
  const question = events[15];
  const cells = buildCells(events.slice(0, 16));
  const context = retrieve("continuitydb", cells, question, 45);

  const prompt = answerPrompt("continuitydb", context, question);

  assert.match(prompt, /authoritative epistemic checkout/i);
  assert.match(prompt, /current belief/i);
  assert.match(prompt, /contradiction/i);
  assert.match(prompt, /active risk/i);
  assert.match(prompt, /do not discard/i);
  assert.ok(prompt.length <= 1250);
});

test("generateAnswer retries empty model responses once", async () => {
  let calls = 0;
  const result = await generateAnswer({
    dryRun: false,
    answerModel: "model",
    ollamaUrl: "http://127.0.0.1:11434",
    prompt: "prompt",
    timeoutMs: 1,
    numPredict: 1,
    deterministicResponse: "dry",
    ollama: async () => {
      calls += 1;
      return calls === 1
        ? { ok: true, model: "model", response: "   ", elapsed_ms: 3, raw: null, error: null }
        : { ok: true, model: "model", response: "real answer", elapsed_ms: 4, raw: null, error: null };
    },
  });

  assert.equal(calls, 2);
  assert.equal(result.ok, true);
  assert.equal(result.response, "real answer");
  assert.equal(result.attempts, 2);
  assert.equal(result.error, null);
});

test("generateAnswer marks repeated empty model responses as failures", async () => {
  const result = await generateAnswer({
    dryRun: false,
    answerModel: "model",
    ollamaUrl: "http://127.0.0.1:11434",
    prompt: "prompt",
    timeoutMs: 1,
    numPredict: 1,
    deterministicResponse: "dry",
    ollama: async () => ({ ok: true, model: "model", response: "", elapsed_ms: 1, raw: null, error: null }),
  });

  assert.equal(result.ok, false);
  assert.equal(result.response, "");
  assert.equal(result.attempts, 2);
  assert.equal(result.error, "empty answer response");
});

test("summarize excludes failed answer cases and reports answer failures by strategy", () => {
  const score = {
    revision_accuracy: 100,
    stale_belief_avoidance: 100,
    uncertainty_calibration: 100,
    hedging_quality: 100,
    surprise_scavenging: 100,
    evidence_grounding: 100,
    hallucinated_certainty: 100,
    overall: 100,
  };
  const summary = summarize([
    { strategy: "continuitydb", answer_ok: false, judges: [{ score }] },
    { strategy: "continuitydb", answer_ok: true, judges: [{ score }] },
    { strategy: "graph", answer_ok: true, judges: [{ score }] },
  ], {
    dryRun: false,
    answerModel: "answer",
    judgeModels: ["judge"],
    scenario: "adversarial",
  });

  assert.equal(summary.by_strategy.continuitydb.scored_count, 1);
  assert.equal(summary.by_strategy.continuitydb.answer_failures, 1);
  assert.equal(summary.answer_failures_by_strategy.continuitydb, 1);
  assert.equal(summary.answer_failures_by_strategy.graph, 0);
});
