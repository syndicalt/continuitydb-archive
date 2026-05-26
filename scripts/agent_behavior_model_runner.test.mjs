import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import test from "node:test";

import {
  buildAgentBehaviorPrompt,
  deterministicAgentBehaviorAnswer,
  runAgentBehaviorModel,
} from "./agent_behavior_model_runner.mjs";

const releaseTask = {
  format: "continuitydb.agent_behavior_runner_input",
  format_version: 1,
  task_id: "release-upload-0",
  trial_index: 0,
  strategy: "continuitydb_checkout",
  prompt: "The GitHub release asset upload failed with HTTP 404 after an older note said the upload succeeded. What should the agent do next?",
  context_packet:
    "Superseding correction: release asset upload failed with HTTP 404. Treat the prior success belief as stale, do not repeat the failed upload command, and verify the release asset URL before retrying.",
  requirements: {
    requires_revision: true,
    requires_uncertainty: false,
    requires_verification: true,
    has_stale_trap: true,
    has_known_failed_action: true,
  },
};

test("buildAgentBehaviorPrompt includes task, context, and scoring requirements", () => {
  const prompt = buildAgentBehaviorPrompt(releaseTask);

  assert.match(prompt, /ContinuityDB agent behavior benchmark/);
  assert.match(prompt, /release-upload-0/);
  assert.match(prompt, /Superseding correction/);
  assert.match(prompt, /requires_revision: true/);
  assert.match(prompt, /Return only the answer text/);
});

test("deterministicAgentBehaviorAnswer follows checkout correction signals", () => {
  const answer = deterministicAgentBehaviorAnswer(releaseTask);

  assert.match(answer, /Do not repeat/i);
  assert.match(answer, /Superseding correction/i);
  assert.match(answer, /verify/i);
  assert.match(answer, /stale/i);
});

test("runAgentBehaviorModel dry-run returns model_output JSON without calling Ollama", async () => {
  let called = false;
  const result = await runAgentBehaviorModel(releaseTask, {
    dryRun: true,
    ollama: async () => {
      called = true;
      throw new Error("dry-run should not call Ollama");
    },
  });

  assert.equal(called, false);
  assert.equal(result.model, "deterministic-dry-run");
  assert.match(result.model_output, /Do not repeat/i);
});

test("runAgentBehaviorModel calls Ollama adapter in live mode", async () => {
  const result = await runAgentBehaviorModel(releaseTask, {
    model: "test-model",
    ollamaUrl: "http://127.0.0.1:11434",
    timeoutMs: 1000,
    numPredict: 128,
    ollama: async ({ url, model, prompt, timeoutMs, numPredict }) => {
      assert.equal(url, "http://127.0.0.1:11434");
      assert.equal(model, "test-model");
      assert.match(prompt, /release-upload-0/);
      assert.equal(timeoutMs, 1000);
      assert.equal(numPredict, 128);
      return {
        ok: true,
        model,
        response: "Use the correction, verify the URL, and avoid repeating the failed command.",
        error: null,
        elapsed_ms: 12,
      };
    },
  });

  assert.equal(result.model, "test-model");
  assert.equal(result.elapsed_ms, 12);
  assert.equal(
    result.model_output,
    "Use the correction, verify the URL, and avoid repeating the failed command.",
  );
});

test("script emits runner protocol JSON in dry-run mode", async () => {
  const stdout = await runScriptWithInput(JSON.stringify(releaseTask));
  const parsed = JSON.parse(stdout);

  assert.match(parsed.model_output, /Do not repeat/i);
  assert.match(parsed.model_output, /verify/i);
});

function runScriptWithInput(input) {
  return new Promise((resolve, reject) => {
    const child = spawn(process.execPath, ["scripts/agent_behavior_model_runner.mjs", "--dry-run"], {
      cwd: process.cwd(),
      stdio: ["pipe", "pipe", "pipe"],
    });
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (chunk) => {
      stdout += chunk;
    });
    child.stderr.on("data", (chunk) => {
      stderr += chunk;
    });
    child.on("error", reject);
    child.on("close", (code) => {
      if (code === 0) resolve(stdout);
      else reject(new Error(stderr || `script exited with status ${code}`));
    });
    child.stdin.end(input);
  });
}
