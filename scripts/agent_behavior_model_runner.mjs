#!/usr/bin/env node
import { performance } from "node:perf_hooks";
import { stdin as processStdin } from "node:process";

const DEFAULT_OLLAMA_URL = "http://127.0.0.1:11434";
const DEFAULT_MODEL = "deepseek-v4-flash:cloud";
const DEFAULT_TIMEOUT_MS = 120000;
const DEFAULT_NUM_PREDICT = 384;

function parseArgs(argv) {
  const args = {
    dryRun: process.env.CONTINUITYDB_AGENT_BEHAVIOR_DRY_RUN === "1",
    model: process.env.CONTINUITYDB_AGENT_BEHAVIOR_MODEL ?? DEFAULT_MODEL,
    ollamaUrl: process.env.OLLAMA_URL ?? DEFAULT_OLLAMA_URL,
    timeoutMs: Number.parseInt(
      process.env.CONTINUITYDB_AGENT_BEHAVIOR_TIMEOUT_MS ?? `${DEFAULT_TIMEOUT_MS}`,
      10,
    ),
    numPredict: Number.parseInt(
      process.env.CONTINUITYDB_AGENT_BEHAVIOR_MAX_TOKENS ?? `${DEFAULT_NUM_PREDICT}`,
      10,
    ),
  };
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    const next = () => argv[++index];
    if (arg === "--dry-run") args.dryRun = true;
    else if (arg === "--model") args.model = next();
    else if (arg === "--ollama-url") args.ollamaUrl = next();
    else if (arg === "--timeout-ms") args.timeoutMs = positiveInteger(next(), arg);
    else if (arg === "--max-tokens") args.numPredict = positiveInteger(next(), arg);
    else if (arg === "--help") {
      printHelp();
      process.exit(0);
    } else {
      throw new Error(`unknown argument: ${arg}`);
    }
  }
  if (!Number.isInteger(args.timeoutMs) || args.timeoutMs <= 0) {
    throw new Error("timeout must be a positive integer");
  }
  if (!Number.isInteger(args.numPredict) || args.numPredict <= 0) {
    throw new Error("max tokens must be a positive integer");
  }
  return args;
}

function positiveInteger(value, flag) {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isInteger(parsed) || parsed <= 0) {
    throw new Error(`${flag} requires a positive integer`);
  }
  return parsed;
}

function printHelp() {
  console.log(`Usage: scripts/agent_behavior_model_runner.mjs [options]

Reads one continuitydb.agent_behavior_runner_input JSON object from stdin and
writes {"model_output": "..."} JSON to stdout.

Options:
  --dry-run              Use deterministic local answer logic, no network calls
  --model MODEL          Ollama model name, default ${DEFAULT_MODEL}
  --ollama-url URL       Ollama base URL, default ${DEFAULT_OLLAMA_URL}
  --timeout-ms N         Ollama request timeout, default ${DEFAULT_TIMEOUT_MS}
  --max-tokens N         Ollama num_predict value, default ${DEFAULT_NUM_PREDICT}
`);
}

export function buildAgentBehaviorPrompt(task) {
  return [
    "ContinuityDB agent behavior benchmark.",
    "",
    `Task id: ${task.task_id}`,
    `Trial index: ${task.trial_index ?? 0}`,
    `Strategy: ${task.strategy}`,
    "",
    "Task prompt:",
    task.prompt,
    "",
    "Context packet:",
    task.context_packet,
    "",
    "Objective scoring requirements:",
    `requires_revision: ${Boolean(task.requirements?.requires_revision)}`,
    `requires_uncertainty: ${Boolean(task.requirements?.requires_uncertainty)}`,
    `requires_verification: ${Boolean(task.requirements?.requires_verification)}`,
    `has_stale_trap: ${Boolean(task.requirements?.has_stale_trap)}`,
    `has_known_failed_action: ${Boolean(task.requirements?.has_known_failed_action)}`,
    "",
    "Return only the answer text. Do not return JSON. Prefer concrete next actions over explanation.",
  ].join("\n");
}

export function deterministicAgentBehaviorAnswer(task) {
  const context = `${task.context_packet ?? ""}`.toLowerCase();
  const requirements = task.requirements ?? {};
  if (context.includes("superseding correction") || context.includes("falsification brief")) {
    return "Do not repeat the failed action. Use the Superseding correction, treat the older belief as stale, and verify the relevant evidence before acting.";
  }
  if (context.includes("uncertainty brief") || requirements.requires_uncertainty) {
    return "The answer should remain uncertain. Verify the source evidence before operational use and avoid a confident claim.";
  }
  if (context.includes("revision capsule") || requirements.requires_revision) {
    return "Use the revised context rather than the older hypothesis. Verify the correction before claiming the result changed.";
  }
  if (requirements.requires_verification) {
    return "Proceed only after verifying the cited evidence and checking the current state.";
  }
  return "Use the provided context packet and complete the task without adding unsupported certainty.";
}

export async function runAgentBehaviorModel(task, options = {}) {
  const dryRun = Boolean(options.dryRun);
  if (dryRun) {
    return {
      model: "deterministic-dry-run",
      model_output: deterministicAgentBehaviorAnswer(task),
      elapsed_ms: 0,
      error: null,
    };
  }

  const model = options.model ?? DEFAULT_MODEL;
  const ollamaUrl = options.ollamaUrl ?? DEFAULT_OLLAMA_URL;
  const timeoutMs = options.timeoutMs ?? DEFAULT_TIMEOUT_MS;
  const numPredict = options.numPredict ?? DEFAULT_NUM_PREDICT;
  const ollama = options.ollama ?? callOllama;
  const result = await ollama({
    url: ollamaUrl,
    model,
    prompt: buildAgentBehaviorPrompt(task),
    timeoutMs,
    numPredict,
  });
  if (!result.ok) {
    throw new Error(result.error ?? `model runner failed for ${model}`);
  }
  return {
    model: result.model ?? model,
    model_output: `${result.response ?? ""}`.trim(),
    elapsed_ms: result.elapsed_ms ?? null,
    error: null,
  };
}

async function callOllama({ url, model, prompt, timeoutMs, numPredict }) {
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
    };
  } catch (error) {
    return {
      ok: false,
      model,
      response: "",
      error: error.message,
      elapsed_ms: Math.round(performance.now() - started),
    };
  } finally {
    clearTimeout(timeout);
  }
}

async function readStdin() {
  const chunks = [];
  for await (const chunk of processStdin) chunks.push(chunk);
  return Buffer.concat(chunks).toString("utf8");
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const input = await readStdin();
  const task = JSON.parse(input);
  const result = await runAgentBehaviorModel(task, args);
  process.stdout.write(`${JSON.stringify({ model_output: result.model_output })}\n`);
}

if (import.meta.url === `file://${process.argv[1]}`) {
  main().catch((error) => {
    process.stderr.write(`${error.message}\n`);
    process.exit(1);
  });
}
