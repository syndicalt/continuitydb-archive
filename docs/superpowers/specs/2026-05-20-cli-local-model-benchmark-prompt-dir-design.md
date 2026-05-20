# CLI Local Model Benchmark Prompt Directory Design

Add first-class prompt artifact export to `benchmark-local-model`.

## Problem

ContinuityDB can export the local Steward response schema and grammar and can materialize those artifacts during benchmark dry-runs and runs. Operators still cannot archive or inspect the exact prompts sent to local model runners for the fixed evaluation suite without reading library internals or wrapping the executable.

That weakens reproducibility for the small embeddable model track. A benchmark baseline records runtime arguments, contract fingerprints, and evaluation results, but not the prompt artifacts an operator can replay or inspect externally.

## Goal

Add `benchmark-local-model --prompt-dir <DIR>`.

When supplied, the benchmark command writes one deterministic prompt file per default evaluation case before dry-run or benchmark execution. The command reports prompt artifact metadata in JSON output:

- case name
- prompt path
- prompt fingerprint
- prompt byte length

Prompt filenames should be deterministic, ordered by evaluation case position, and safe for common filesystems.

## Boundaries

- Do not run a real local model in tests.
- Do not change benchmark scoring or the default evaluation cases.
- Do not duplicate prompt construction logic in the CLI; expose a steward-level prompt rendering helper and call that from the CLI.
- Do not record prompts inside durable baseline records yet; this slice records artifact metadata in the current CLI output.

## Acceptance Criteria

- `benchmark-local-model --dry-run --prompt-dir <DIR>` writes one prompt file per default evaluation case.
- Prompt artifact JSON includes each case name, path, fingerprint, and byte length.
- The first prompt includes the Steward role instruction, task, evidence locator, and evidence text.
- Existing dry-run behavior without `--prompt-dir` reports `prompt_artifacts: []`.
- README and roadmap document the new command path.
