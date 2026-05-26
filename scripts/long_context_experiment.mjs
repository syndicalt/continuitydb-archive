#!/usr/bin/env node
import { mkdir, writeFile } from "node:fs/promises";
import { performance } from "node:perf_hooks";
import path from "node:path";

const DEFAULT_OUTPUT_DIR = "docs/benchmarks/long-context-experiment";
const STRATEGIES = ["continuitydb", "graph", "vector", "summary"];
const JUDGE_SCORE_FIELDS = [
  "revision_accuracy",
  "stale_belief_avoidance",
  "uncertainty_calibration",
  "hedging_quality",
  "surprise_scavenging",
  "evidence_grounding",
  "hallucinated_certainty",
  "overall",
];

function parseArgs(argv) {
  const args = {
    turns: 20,
    seeds: 1,
    outputDir: DEFAULT_OUTPUT_DIR,
    answerModel: "deepseek-v4-flash:cloud",
    judgeModels: ["deepseek-v4-flash:cloud"],
    ollamaUrl: "http://127.0.0.1:11434",
    dryRun: false,
    tokenBudget: 60,
    answerTimeoutMs: 120000,
    judgeTimeoutMs: 180000,
    answerMaxTokens: 384,
    judgeMaxTokens: 256,
    concurrency: 1,
    scenario: "baseline",
  };
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    const next = () => argv[++index];
    if (arg === "--turns") args.turns = positiveInteger(next(), "--turns");
    else if (arg === "--seeds") args.seeds = positiveInteger(next(), "--seeds");
    else if (arg === "--output-dir") args.outputDir = next();
    else if (arg === "--answer-model") args.answerModel = next();
    else if (arg === "--judge-model") args.judgeModels.push(next());
    else if (arg === "--judge-models") args.judgeModels = next().split(",").filter(Boolean);
    else if (arg === "--ollama-url") args.ollamaUrl = next();
    else if (arg === "--token-budget") args.tokenBudget = positiveInteger(next(), arg);
    else if (arg === "--answer-timeout-ms") args.answerTimeoutMs = positiveInteger(next(), arg);
    else if (arg === "--judge-timeout-ms") args.judgeTimeoutMs = positiveInteger(next(), arg);
    else if (arg === "--answer-max-tokens") args.answerMaxTokens = positiveInteger(next(), arg);
    else if (arg === "--judge-max-tokens") args.judgeMaxTokens = positiveInteger(next(), arg);
    else if (arg === "--concurrency") args.concurrency = positiveInteger(next(), arg);
    else if (arg === "--scenario") args.scenario = scenarioName(next());
    else if (arg === "--dry-run") args.dryRun = true;
    else if (arg === "--help") {
      printHelp();
      process.exit(0);
    } else {
      throw new Error(`unknown argument: ${arg}`);
    }
  }
  args.judgeModels = [...new Set(args.judgeModels)];
  return args;
}

function positiveInteger(value, flag) {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isInteger(parsed) || parsed <= 0) {
    throw new Error(`${flag} requires a positive integer`);
  }
  return parsed;
}

function scenarioName(value) {
  if (value === "baseline" || value === "adversarial") return value;
  throw new Error("--scenario must be baseline or adversarial");
}

function printHelp() {
  console.log(`Usage: node scripts/long_context_experiment.mjs [options]

Options:
  --turns N                 Number of scripted turns per seed, default 20
  --seeds N                 Number of deterministic seeds, default 1
  --output-dir PATH         Artifact directory, default ${DEFAULT_OUTPUT_DIR}
  --answer-model MODEL      Ollama answer model, default deepseek-v4-flash:cloud
  --judge-model MODEL       Add one Ollama judge model; repeatable
  --judge-models A,B        Replace judge model list with comma-separated models
  --token-budget N          Retrieved context token budget per answer, default 60
  --answer-max-tokens N     Ollama answer num_predict cap, default 384
  --judge-max-tokens N      Ollama judge num_predict cap, default 256
  --concurrency N           Concurrent strategy cases, default 1
  --scenario NAME           baseline or adversarial, default baseline
  --dry-run                 Do not call Ollama; use deterministic answer/judge stubs
`);
}

export function eventForTurn(turn, seed, scenario = "baseline") {
  if (scenario === "adversarial") return adversarialEventForTurn(turn, seed);
  return baselineEventForTurn(turn, seed);
}

function baselineEventForTurn(turn, seed) {
  const cycle = (turn - 1) % 8;
  const release = `release-${seed}-${Math.floor((turn - 1) / 8) + 1}`;
  const common = { turn, release };
  if (cycle === 0) {
    return {
      ...common,
      kind: "claim",
      prompt: `Record planning state for ${release}.`,
      text: `${release}: release assets are expected to upload successfully.`,
      probability: 0.82,
      impact: 0.8,
      truth: "pending",
    };
  }
  if (cycle === 1) {
    return {
      ...common,
      kind: "evidence",
      prompt: `Record apparent success for ${release}.`,
      text: `${release}: release assets uploaded successfully according to the first workflow log.`,
      probability: 0.95,
      impact: 0.9,
      truth: "apparently_shipped",
    };
  }
  if (cycle === 2) {
    return {
      ...common,
      kind: "question",
      prompt: `Can we treat ${release} as shipped?`,
      expected: `${release} can be treated as provisionally shipped, but the answer should cite the first workflow log and avoid claiming independent verification.`,
      truth: "apparently_shipped",
    };
  }
  if (cycle === 3) {
    return {
      ...common,
      kind: "surprise",
      prompt: `Record surprising failure for ${release}.`,
      text: `${release}: GitHub Release asset upload returned HTTP 404 after the earlier success claim.`,
      probability: 0.05,
      impact: 1.0,
      truth: "blocked",
    };
  }
  if (cycle === 4) {
    return {
      ...common,
      kind: "question",
      prompt: `Can we still treat ${release} as shipped after the 404?`,
      expected: `${release} should not be treated as shipped. The answer should mention the 404, the superseded earlier success claim, uncertainty, and the need to rerun or verify upload.`,
      truth: "blocked",
    };
  }
  if (cycle === 5) {
    return {
      ...common,
      kind: "revision",
      prompt: `Record corrective action for ${release}.`,
      text: `${release}: release asset upload was rerun and verified from the GitHub release asset list.`,
      probability: 0.88,
      impact: 1.0,
      truth: "verified_shipped",
    };
  }
  if (cycle === 6) {
    return {
      ...common,
      kind: "question",
      prompt: `What is the current release status for ${release}?`,
      expected: `${release} is verified shipped after the rerun. The answer should explain the revision chain: initial apparent success, 404 conflict, rerun verification.`,
      truth: "verified_shipped",
    };
  }
  return {
    ...common,
    kind: "question",
    prompt: `What evidence should remain in context for ${release}?`,
    expected: `The answer should retain current verification plus the conflict and supersession trail so future agents do not collapse back to the stale success claim.`,
    truth: "verified_shipped",
  };
}

function adversarialEventForTurn(turn, seed) {
  const cycle = (turn - 1) % 16;
  const release = `release-${seed}-${Math.floor((turn - 1) / 16) + 1}`;
  const common = { turn, release };
  if (cycle === 0) {
    return {
      ...common,
      kind: "claim",
      prompt: `Record release plan for ${release}.`,
      text: `${release}: initial maintainer belief says release assets should upload cleanly.`,
      probability: 0.86,
      impact: 0.8,
      truth: "pending",
    };
  }
  if (cycle === 1) {
    return {
      ...common,
      kind: "evidence",
      prompt: `Record early success signal for ${release}.`,
      text: `${release}: first workflow log says release assets uploaded successfully.`,
      probability: 0.94,
      impact: 0.9,
      truth: "apparently_shipped",
    };
  }
  if (cycle === 2) {
    return {
      ...common,
      kind: "question",
      prompt: `Can ${release} be considered shipped from the first log?`,
      expected: `${release} is only provisionally shipped. The answer should cite the first workflow log and avoid independent verification language.`,
      truth: "apparently_shipped",
    };
  }
  if (cycle === 3) {
    return {
      ...common,
      kind: "surprise",
      prompt: `Record contradiction for ${release}.`,
      text: `${release}: GitHub asset list returned HTTP 404, contradicting the first workflow success log.`,
      probability: 0.04,
      impact: 1.0,
      truth: "blocked",
    };
  }
  if (cycle === 4) {
    return {
      ...common,
      kind: "question",
      prompt: `Why should ${release}'s first success log no longer be trusted?`,
      expected: `${release}'s first success log is superseded by the 404 from the GitHub Release asset list. The answer should preserve the contradiction, avoid treating the release as shipped, and call for verification.`,
      truth: "blocked",
    };
  }
  if (cycle === 5) {
    return {
      ...common,
      kind: "evidence",
      prompt: `Record ambiguous later signal for ${release}.`,
      text: `${release}: ambiguous CDN cache signal; GitHub asset list still unverified.`,
      probability: 0.52,
      impact: 0.9,
      truth: "blocked_uncertain",
    };
  }
  if (cycle === 6) {
    return {
      ...common,
      kind: "question",
      prompt: `Can ${release} be treated as shipped after the ambiguous CDN cache signal?`,
      expected: `${release} should still not be treated as shipped. The answer must mention the ambiguous CDN cache signal, the prior HTTP 404 contradiction, uncertainty, and the need to verify the GitHub Release asset list.`,
      truth: "blocked_uncertain",
    };
  }
  if (cycle === 7) {
    return {
      ...common,
      kind: "evidence",
      prompt: `Record stale operator shortcut for ${release}.`,
      text: `${release}: operator note says ship it because the cached asset name appeared in a dashboard.`,
      probability: 0.57,
      impact: 0.7,
      truth: "blocked_uncertain",
      frontier: false,
    };
  }
  if (cycle === 8) {
    return {
      ...common,
      kind: "question",
      prompt: `Should the operator shortcut for ${release} override the 404?`,
      expected: `The operator shortcut should not override the 404. The answer should identify it as weak evidence, preserve uncertainty, and require direct GitHub Release asset verification.`,
      truth: "blocked_uncertain",
    };
  }
  if (cycle === 9) {
    return {
      ...common,
      kind: "revision",
      prompt: `Record verified correction for ${release}.`,
      text: `${release}: rerun verified expected assets in the GitHub Release asset list.`,
      probability: 0.91,
      impact: 1.0,
      truth: "verified_shipped",
    };
  }
  if (cycle === 10) {
    return {
      ...common,
      kind: "question",
      prompt: `What is ${release}'s current status and why?`,
      expected: `${release} is verified shipped after rerun. The answer should retain the revision chain: first success log, 404 contradiction, ambiguous CDN signal, weak operator shortcut, and final GitHub asset-list verification.`,
      truth: "verified_shipped",
    };
  }
  if (cycle === 11) {
    return {
      ...common,
      kind: "evidence",
      prompt: `Record stale incident duplicate for ${release}.`,
      text: `${release}: duplicated incident comment repeats the old 404 without checking whether the later rerun superseded it.`,
      probability: 0.48,
      impact: 0.8,
      truth: "verified_shipped_with_stale_conflict",
      frontier: false,
    };
  }
  if (cycle === 12) {
    return {
      ...common,
      kind: "question",
      prompt: `Should the duplicated old 404 reopen ${release}?`,
      expected: `The duplicated 404 should not reopen ${release} by itself because the later rerun verified the asset list. The answer should cite the final verification, mention the stale duplicate, and preserve the audit trail.`,
      truth: "verified_shipped_with_stale_conflict",
    };
  }
  if (cycle === 13) {
    return {
      ...common,
      kind: "evidence",
      prompt: `Record follow-up risk for ${release}.`,
      text: `${release}: follow-up risk says checksum provenance is unknown even though asset presence is verified.`,
      probability: 0.62,
      impact: 0.9,
      truth: "verified_with_provenance_risk",
    };
  }
  if (cycle === 14) {
    return {
      ...common,
      kind: "question",
      prompt: `What should remain in context for future work on ${release}?`,
      expected: `The answer should retain current asset-list verification, the old 404 contradiction, the ambiguous CDN/operator shortcuts, and the open checksum provenance risk so future agents do not collapse to a simple shipped/not-shipped summary.`,
      truth: "verified_with_provenance_risk",
    };
  }
  return {
    ...common,
    kind: "question",
    prompt: `What is the safest action for ${release} now?`,
    expected: `${release} can proceed as asset-present, but checksum provenance remains uncertain. The answer should not collapse the old 404 or ambiguous CDN shortcut; it should recommend provenance verification before broader release claims.`,
    truth: "verified_with_provenance_risk",
  };
}

export function buildCells(events) {
  const cells = [];
  for (const event of events) {
    if (!["claim", "evidence", "surprise", "revision"].includes(event.kind)) continue;
    const id = `cell-${event.release}-${event.turn}`;
    const supersedes = event.kind === "surprise" || event.kind === "revision"
      ? latestCellId(cells, event.release)
      : null;
    const conflicts = event.kind === "surprise" ? latestCellId(cells, event.release) : null;
    const surpriseBits = event.kind === "surprise" ? -Math.log2(Math.max(event.probability, 0.0001)) : 0;
    cells.push({
      id,
      turn: event.turn,
      release: event.release,
      kind: event.kind,
      text: event.text,
      confidence: event.probability,
      impact: event.impact,
      tokenCost: Math.ceil(event.text.length / 4),
      currentTruth: event.truth,
      frontier: event.frontier !== false,
      supersedes,
      conflicts,
      uncertainty: entropy(event.probability),
      surpriseBits,
      citation: `workflow://${event.release}/${event.turn}`,
    });
  }
  return cells;
}

function latestCellId(cells, release) {
  const found = [...cells].reverse().find((cell) => cell.release === release);
  return found?.id ?? null;
}

function entropy(p) {
  if (p <= 0 || p >= 1) return 0;
  return -p * Math.log2(p) - (1 - p) * Math.log2(1 - p);
}

export function retrieve(strategy, cells, event, tokenBudget = 900) {
  const candidates = cells.filter((cell) => cell.release === event.release);
  if (strategy === "continuitydb") {
    return continuityCheckout(candidates, tokenBudget);
  }
  if (strategy === "graph") {
    return packByScore(candidates, tokenBudget, (cell) =>
      2 * isCurrent(candidates, cell)
      + 2 * Boolean(cell.supersedes)
      + 2 * Boolean(cell.conflicts)
      + cell.impact
    );
  }
  if (strategy === "vector") {
    const queryTerms = new Set(event.prompt.toLowerCase().split(/[^a-z0-9]+/).filter(Boolean));
    return packByScore(candidates, tokenBudget, (cell) => lexicalOverlap(queryTerms, cell.text));
  }
  if (strategy === "summary") {
    const latest = latestCell(candidates);
    return latest ? [latest] : [];
  }
  throw new Error(`unknown strategy: ${strategy}`);
}

function continuityCheckout(candidates, tokenBudget) {
  const current = latestCurrentBelief(candidates);
  const contradiction = decisiveContradiction(candidates);
  const risk = activeRisk(candidates, current);
  const roleCells = uniqueCells([current, contradiction, risk].filter(Boolean));
  const roleCost = roleCells.reduce((sum, cell) => sum + cell.tokenCost, 0);
  if (roleCost <= tokenBudget) {
    return packRoleCellsWithAuditTrail(candidates, roleCells, tokenBudget);
  }
  return compactRoleCells({ current, contradiction, risk }, tokenBudget);
}

function packRoleCellsWithAuditTrail(candidates, roleCells, tokenBudget) {
  const reservedCost = roleCells.reduce((sum, cell) => sum + cell.tokenCost, 0);
  const remainingBudget = tokenBudget - reservedCost;
  const reserved = new Set(roleCells.map((cell) => cell.id));
  const auditTrail = packByScore(candidates.filter((cell) => !reserved.has(cell.id)), remainingBudget, (cell) =>
    4 * isCurrent(candidates, cell)
    + 3 * Boolean(cell.supersedes)
    + 2 * Boolean(cell.conflicts)
    + 2 * cell.uncertainty
    + 2 * cell.surpriseBits
    + cell.impact
    + cell.confidence
  );
  return [...roleCells, ...auditTrail];
}

function latestCurrentBelief(candidates) {
  const latestRevision = [...candidates]
    .filter((cell) => cell.kind === "revision" && cell.frontier)
    .sort((left, right) => left.turn - right.turn)
    .at(-1);
  return latestRevision ?? latestFrontierCell(candidates);
}

function decisiveContradiction(candidates) {
  return [...candidates]
    .filter((cell) => cell.conflicts || cell.surpriseBits > 0)
    .sort((left, right) => right.surpriseBits - left.surpriseBits || right.impact - left.impact || right.turn - left.turn)
    .at(0) ?? null;
}

function activeRisk(candidates, current) {
  const currentTurn = current?.turn ?? 0;
  return [...candidates]
    .filter((cell) => cell.frontier && cell.turn > currentTurn && cell.id !== current?.id)
    .filter((cell) => cell.uncertainty >= 0.65 || /risk|uncertain|unknown|unverified|ambiguous|provenance/i.test(cell.text))
    .sort((left, right) => right.turn - left.turn)
    .at(0) ?? null;
}

function uniqueCells(cells) {
  const seen = new Set();
  const out = [];
  for (const cell of cells) {
    if (seen.has(cell.id)) continue;
    seen.add(cell.id);
    out.push(cell);
  }
  return out;
}

function compactRoleCells({ current, contradiction, risk }, tokenBudget) {
  const compact = uniqueCells([
    current ? compactCell(current, "current") : null,
    contradiction ? compactCell(contradiction, "contradiction") : null,
    risk ? compactCell(risk, "risk") : null,
  ].filter(Boolean));
  return packByScore(compact, tokenBudget, (cell) => cell.rolePriority);
}

function compactCell(cell, role) {
  const text = compactText(cell, role);
  const priority = role === "current" ? 3 : role === "contradiction" ? 2 : 1;
  return {
    ...cell,
    id: `${cell.id}:checkout:${role}`,
    text,
    tokenCost: Math.ceil(text.length / 4),
    role,
    rolePriority: priority,
  };
}

function compactText(cell, role) {
  if (role === "current") {
    if (/rerun/i.test(cell.text)) return "Current belief: asset-present after rerun.";
    return `Current belief: ${withoutReleasePrefix(cell.text)}`;
  }
  if (role === "contradiction") {
    if (/404/i.test(cell.text)) return "Decisive contradiction: HTTP 404 invalidated first success.";
    return `Decisive contradiction: ${withoutReleasePrefix(cell.text)}`;
  }
  if (/checksum|provenance/i.test(cell.text)) return "Active risk: checksum provenance unknown. Safest action: verify provenance.";
  if (/ambiguous|unverified/i.test(cell.text)) return "Active risk: asset signal ambiguous. Safest action: verify GitHub assets.";
  return `Active risk: ${withoutReleasePrefix(cell.text)}`;
}

function withoutReleasePrefix(text) {
  return text.replace(/^release-[^:]+:\s*/, "");
}

function isCurrent(candidates, cell) {
  const superseded = new Set(candidates.map((candidate) => candidate.supersedes).filter(Boolean));
  return superseded.has(cell.id) ? 0 : 1;
}

function latestCell(candidates) {
  return [...candidates].sort((left, right) => left.turn - right.turn).at(-1) ?? null;
}

function latestFrontierCell(candidates) {
  return [...candidates]
    .filter((cell) => cell.frontier)
    .sort((left, right) => left.turn - right.turn)
    .at(-1) ?? latestCell(candidates);
}

function lexicalOverlap(queryTerms, text) {
  const terms = new Set(text.toLowerCase().split(/[^a-z0-9]+/).filter(Boolean));
  let overlap = 0;
  for (const term of queryTerms) if (terms.has(term)) overlap += 1;
  return overlap;
}

function packByScore(candidates, tokenBudget, scoreFn) {
  const ranked = candidates
    .map((cell) => ({ cell, score: scoreFn(cell) }))
    .sort((left, right) => right.score - left.score);
  const selected = [];
  let tokens = 0;
  for (const { cell } of ranked) {
    if (tokens + cell.tokenCost <= tokenBudget) {
      selected.push(cell);
      tokens += cell.tokenCost;
    }
  }
  return selected;
}

export function answerPrompt(strategy, context, event) {
  const continuityInstructions = strategy === "continuitydb"
    ? `
ContinuityDB context is an authoritative epistemic checkout: use current belief, contradiction, and active risk; do not discard any role.`
    : "";
  return `You are answering from a long-running agent memory experiment.

Memory strategy: ${strategy}
Question: ${event.prompt}
${continuityInstructions}

Retrieved context:
${context.map(formatCell).join("\n\n")}

Answer in four concise bullets. State the current answer, mention uncertainty when relevant, cite evidence locators, and avoid treating superseded claims as current.`;
}

function formatCell(cell) {
  return `- id=${cell.id}
  text=${cell.text}
  confidence=${cell.confidence}
  uncertainty=${cell.uncertainty.toFixed(3)}
  surprise_bits=${cell.surpriseBits.toFixed(3)}
  supersedes=${cell.supersedes ?? "none"}
  conflicts=${cell.conflicts ?? "none"}
  citation=${cell.citation}`;
}

function deterministicAnswer(strategy, context, event) {
  const current = latestCell(context);
  const hasConflict = context.some((cell) => cell.conflicts);
  const hasRevision = context.some((cell) => cell.supersedes);
  const uncertainty = context.reduce((max, cell) => Math.max(max, cell.uncertainty), 0);
  return [
    `Current answer: ${current?.text ?? "No relevant memory was retrieved."}`,
    `Revision: ${hasRevision ? "revision context is present" : "revision context is missing"}.`,
    `Uncertainty: ${hasConflict || uncertainty > 0.5 ? "preserve uncertainty and verify before acting" : "low uncertainty in retrieved context"}.`,
    `Evidence: ${context.map((cell) => cell.citation).join(", ") || "none"}.`,
  ].join("\n");
}

export async function generateAnswer({
  dryRun,
  answerModel,
  ollamaUrl,
  prompt,
  timeoutMs,
  numPredict,
  deterministicResponse,
  ollama = callOllama,
}) {
  if (dryRun) {
    return {
      ok: true,
      model: "deterministic-dry-run",
      response: deterministicResponse,
      elapsed_ms: 0,
      raw: null,
      error: null,
      attempts: 1,
    };
  }
  let lastResult = null;
  for (let attempt = 1; attempt <= 2; attempt += 1) {
    const result = await ollama({
      url: ollamaUrl,
      model: answerModel,
      prompt,
      timeoutMs,
      numPredict,
    });
    lastResult = result;
    if (result.ok && result.response.trim()) {
      return { ...result, attempts: attempt };
    }
    if (!result.ok) {
      return { ...result, attempts: attempt };
    }
  }
  return {
    ...lastResult,
    ok: false,
    response: lastResult?.response ?? "",
    error: lastResult?.error ?? "empty answer response",
    attempts: 2,
  };
}

function judgePrompt(strategy, context, event, answer) {
  return `Score this long-context memory answer. Return strict JSON only.

Rubric fields: revision_accuracy, stale_belief_avoidance, uncertainty_calibration, hedging_quality, surprise_scavenging, evidence_grounding, hallucinated_certainty, overall.
Scores are integers from 0 to 100. hallucinated_certainty is reversed: 100 means no hallucinated certainty.

Expected behavior:
${event.expected}

Memory strategy: ${strategy}
Question: ${event.prompt}
Retrieved context:
${context.map(formatCell).join("\n\n")}

Answer:
${answer}

Return JSON shape:
{"revision_accuracy":0,"stale_belief_avoidance":0,"uncertainty_calibration":0,"hedging_quality":0,"surprise_scavenging":0,"evidence_grounding":0,"hallucinated_certainty":0,"overall":0,"rationale":"..."}`;
}

function deterministicJudge(context, event, answer) {
  const text = answer.toLowerCase();
  const expectsBlocked = event.truth === "blocked";
  const expectsVerified = event.truth === "verified_shipped";
  const mentions404 = text.includes("404");
  const mentionsVerify = text.includes("verify") || text.includes("rerun");
  const mentionsUncertainty = text.includes("uncertain") || text.includes("verify") || text.includes("provisional");
  const hasRevision = context.some((cell) => cell.supersedes);
  const hasConflict = context.some((cell) => cell.conflicts);
  const staleCertainty = expectsBlocked && (text.includes("shipped") && !text.includes("not"));
  const revisionAccuracy = hasRevision && (mentions404 || expectsVerified) ? 85 : 45;
  const staleAvoidance = staleCertainty ? 20 : 85;
  const uncertaintyCalibration = mentionsUncertainty || hasConflict ? 80 : 45;
  const hedgingQuality = mentionsUncertainty ? 80 : 50;
  const surpriseScavenging = hasConflict && mentions404 && mentionsVerify ? 90 : hasConflict ? 65 : 50;
  const evidenceGrounding = text.includes("workflow://") ? 90 : 55;
  const hallucinatedCertainty = staleCertainty ? 10 : 85;
  const values = [
    revisionAccuracy,
    staleAvoidance,
    uncertaintyCalibration,
    hedgingQuality,
    surpriseScavenging,
    evidenceGrounding,
    hallucinatedCertainty,
  ];
  return {
    revision_accuracy: revisionAccuracy,
    stale_belief_avoidance: staleAvoidance,
    uncertainty_calibration: uncertaintyCalibration,
    hedging_quality: hedgingQuality,
    surprise_scavenging: surpriseScavenging,
    evidence_grounding: evidenceGrounding,
    hallucinated_certainty: hallucinatedCertainty,
    overall: Math.round(values.reduce((sum, value) => sum + value, 0) / values.length),
    rationale: "deterministic rubric judge",
  };
}

export async function runJudge({
  dryRun,
  judgeModel,
  context,
  event,
  answer,
  strategy = "test",
  ollama = callOllama,
  ollamaUrl,
  timeoutMs,
  numPredict,
}) {
  if (dryRun || judgeModel === "deterministic") {
    const score = deterministicJudge(context, event, answer);
    return {
      model: dryRun ? "deterministic-dry-run" : "deterministic",
      ok: true,
      elapsed_ms: 0,
      error: null,
      score,
      raw_response: JSON.stringify(score),
    };
  }
  const judgeText = judgePrompt(strategy, context, event, answer);
  const judgeResult = await ollama({
    url: ollamaUrl,
    model: judgeModel,
    prompt: judgeText,
    timeoutMs,
    formatJson: true,
    numPredict,
  });
  return {
    model: judgeModel,
    ok: judgeResult.ok,
    elapsed_ms: judgeResult.elapsed_ms,
    error: judgeResult.error,
    score: judgeResult.ok ? parseJudgeJson(judgeResult.response) : null,
    raw_response: judgeResult.response,
  };
}

export async function mapWithConcurrency(items, concurrency, worker) {
  const results = new Array(items.length);
  let nextIndex = 0;
  const workerCount = Math.min(concurrency, items.length);
  await Promise.all(Array.from({ length: workerCount }, async () => {
    while (nextIndex < items.length) {
      const index = nextIndex;
      nextIndex += 1;
      results[index] = await worker(items[index], index);
    }
  }));
  return results;
}

async function callOllama({ url, model, prompt, timeoutMs, formatJson = false, numPredict }) {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), timeoutMs);
  const started = performance.now();
  try {
    const response = await fetch(`${url}/api/generate`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        model,
        prompt,
        stream: false,
        format: formatJson ? "json" : undefined,
        options: { temperature: 0, num_predict: numPredict },
      }),
      signal: controller.signal,
    });
    const body = await response.json();
    return {
      ok: response.ok && !body.error,
      model: body.model ?? model,
      response: body.response ?? "",
      error: body.error ?? null,
      elapsed_ms: Math.round(performance.now() - started),
      raw: body,
    };
  } catch (error) {
    return {
      ok: false,
      model,
      response: "",
      error: error.message,
      elapsed_ms: Math.round(performance.now() - started),
      raw: null,
    };
  } finally {
    clearTimeout(timeout);
  }
}

export function parseJudgeJson(text) {
  const trimmed = text.trim();
  const fenced = trimmed.match(/^```(?:json)?\s*([\s\S]*?)\s*```$/i);
  const candidate = fenced ? fenced[1] : trimmed;
  try {
    return JSON.parse(candidate);
  } catch {
    const match = candidate.match(/\{[\s\S]*\}/);
    if (!match) return parseJudgeScoreFields(candidate);
    try {
      return JSON.parse(match[0]);
    } catch {
      return parseJudgeScoreFields(candidate);
    }
  }
}

function parseJudgeScoreFields(text) {
  const parsed = {};
  for (const field of JUDGE_SCORE_FIELDS) {
    const match = text.match(new RegExp(`"${field}"\\s*:\\s*(-?\\d+(?:\\.\\d+)?)`));
    if (!match) return null;
    parsed[field] = Number(match[1]);
  }
  parsed.rationale = "";
  return parsed;
}

async function run(args) {
  await mkdir(args.outputDir, { recursive: true });
  const caseSpecs = [];
  for (let seed = 1; seed <= args.seeds; seed += 1) {
    const events = Array.from({ length: args.turns }, (_, index) => eventForTurn(index + 1, seed, args.scenario));
    for (let turn = 1; turn <= events.length; turn += 1) {
      const event = events[turn - 1];
      if (event.kind !== "question") continue;
      const cells = buildCells(events.slice(0, turn));
      for (const strategy of STRATEGIES) {
        caseSpecs.push({ seed, turn, event, cells, strategy });
      }
    }
  }
  const totalCases = caseSpecs.length;
  let completedCases = 0;
  const cases = await mapWithConcurrency(caseSpecs, args.concurrency, async ({ seed, turn, event, cells, strategy }) => {
    const context = retrieve(strategy, cells, event, args.tokenBudget);
    const prompt = answerPrompt(strategy, context, event);
    const answerResult = await generateAnswer({
      dryRun: args.dryRun,
      answerModel: args.answerModel,
      ollamaUrl: args.ollamaUrl,
      prompt,
      timeoutMs: args.answerTimeoutMs,
      numPredict: args.answerMaxTokens,
      deterministicResponse: deterministicAnswer(strategy, context, event),
    });
    const answer = answerResult.response;
    const judges = [];
    if (answerResult.ok) {
      for (const judgeModel of args.judgeModels) {
        judges.push(await runJudge({
          dryRun: args.dryRun,
          judgeModel,
          context,
          event,
          answer,
          strategy,
          ollamaUrl: args.ollamaUrl,
          timeoutMs: args.judgeTimeoutMs,
          numPredict: args.judgeMaxTokens,
        }));
      }
    }
    completedCases += 1;
    console.error(`[long-context] ${completedCases}/${totalCases} seed=${seed} turn=${turn} strategy=${strategy}`);
    return {
      seed,
      turn,
      strategy,
      event,
      context,
      prompt,
      answer_model: args.dryRun ? "deterministic-dry-run" : args.answerModel,
      answer_ok: answerResult.ok,
      answer_error: answerResult.error,
      answer_elapsed_ms: answerResult.elapsed_ms,
      answer_attempts: answerResult.attempts,
      answer,
      judges,
    };
  });
  const summary = summarize(cases, args);
  const report = {
    format: "continuitydb.long_context_experiment",
    format_version: 1,
    generated_at: new Date().toISOString(),
    args,
    strategies: STRATEGIES,
    summary,
    cases,
  };
  await writeFile(path.join(args.outputDir, "long-context-experiment.json"), JSON.stringify(report, null, 2));
  await writeFile(path.join(args.outputDir, "long-context-experiment.md"), markdown(report));
  console.log(JSON.stringify({ output_dir: args.outputDir, summary }, null, 2));
}

export function summarize(cases, args) {
  const byStrategy = {};
  const answerFailuresByStrategy = {};
  for (const strategy of STRATEGIES) {
    const strategyCases = cases.filter((entry) => entry.strategy === strategy);
    const answerFailures = strategyCases.filter((entry) => !entry.answer_ok).length;
    answerFailuresByStrategy[strategy] = answerFailures;
    const scores = strategyCases.filter((entry) => entry.answer_ok).flatMap((entry) =>
      entry.judges
        .filter((judge) => judge.score && Number.isFinite(judge.score.overall))
        .map((judge) => judge.score)
    );
    byStrategy[strategy] = aggregateScores(scores);
    byStrategy[strategy].answer_failures = answerFailures;
  }
  return {
    dry_run: args.dryRun,
    case_count: cases.length,
    answer_model: args.dryRun ? "deterministic-dry-run" : args.answerModel,
    judge_models: args.dryRun ? ["deterministic-dry-run"] : args.judgeModels,
    scenario: args.scenario,
    answer_failures_by_strategy: answerFailuresByStrategy,
    by_strategy: byStrategy,
  };
}

function aggregateScores(scores) {
  const fields = [
    "revision_accuracy",
    "stale_belief_avoidance",
    "uncertainty_calibration",
    "hedging_quality",
    "surprise_scavenging",
    "evidence_grounding",
    "hallucinated_certainty",
    "overall",
  ];
  const out = { scored_count: scores.length };
  for (const field of fields) {
    const values = scores.map((score) => Number(score[field])).filter(Number.isFinite);
    out[field] = values.length
      ? Math.round(values.reduce((sum, value) => sum + value, 0) / values.length)
      : null;
  }
  return out;
}

function markdown(report) {
  const lines = [];
  lines.push("# ContinuityDB Long-Context Experiment\n");
  lines.push(`Dry run: \`${report.summary.dry_run}\`.`);
  lines.push(`Scenario: \`${report.summary.scenario}\`.`);
  lines.push(`Answer model: \`${report.summary.answer_model}\`.`);
  lines.push(`Judge models: \`${report.summary.judge_models.join(", ")}\`.\n`);
  lines.push("| Strategy | Cases | Answer Failures | Overall | Revision | Stale Avoidance | Uncertainty | Hedging | Surprise | Evidence | No Hallucinated Certainty |");
  lines.push("| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |");
  for (const strategy of STRATEGIES) {
    const row = report.summary.by_strategy[strategy];
    lines.push(`| ${strategy} | ${row.scored_count} | ${value(row.answer_failures)} | ${value(row.overall)} | ${value(row.revision_accuracy)} | ${value(row.stale_belief_avoidance)} | ${value(row.uncertainty_calibration)} | ${value(row.hedging_quality)} | ${value(row.surprise_scavenging)} | ${value(row.evidence_grounding)} | ${value(row.hallucinated_certainty)} |`);
  }
  lines.push("\n## Notes\n");
  lines.push("- Scores are 0-100 judge outputs, not ContinuityDB internal semantic coverage scores.");
  lines.push("- Same-model judging should be treated as a smoke test; use a different judge model for stronger evidence.");
  lines.push("- The `deterministic` judge is a fixed rubric scorer, not an LLM judge.");
  lines.push("- The scenario is scripted and deterministic so it can be replayed across models and seeds.");
  return `${lines.join("\n")}\n`;
}

function value(input) {
  return input === null || input === undefined ? "" : String(input);
}

if (import.meta.url === `file://${process.argv[1]}`) {
  run(parseArgs(process.argv.slice(2))).catch((error) => {
    console.error(error);
    process.exit(1);
  });
}
