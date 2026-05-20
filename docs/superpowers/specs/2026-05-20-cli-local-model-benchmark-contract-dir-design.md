# CLI Local Model Benchmark Contract Directory Design

Add a first-class contract artifact directory to `benchmark-local-model`.

## Problem

The CLI can export the local Steward response schema and GBNF grammar, pass a grammar path to benchmark runs, and reject strict candidate configurations that omit a required grammar. Operators still need a multi-command workflow to produce the contract files before a strict dry-run or benchmark can prove the executable arguments point at the exact grammar artifact for the current binary.

## Goal

Add `benchmark-local-model --contract-dir <DIR>`.

When supplied, the benchmark command writes:

- `local-model-response.schema.json`
- `local-model-response.gbnf`

to the selected directory before dry-run or real benchmark execution. The command reports both artifact paths and deterministic fingerprints in output. If no explicit `--grammar-path` is supplied, the generated grammar path becomes the runtime grammar argument. If `--grammar-path` is supplied, it remains the explicit runtime choice while the generated contract artifacts are still reported.

## Boundaries

- Do not run a real local model in tests.
- Do not change the local model response schema, grammar, evaluation suite, or scoring.
- Do not weaken `--enforce-candidate-requirements`.
- Do not make model output mutate committed truth directly.
- Use deterministic filenames.

## Acceptance Criteria

- `benchmark-local-model --dry-run --candidate-defaults --contract-dir <DIR> --enforce-candidate-requirements` succeeds for a grammar-required candidate without explicit `--grammar-path`.
- The dry-run writes schema and grammar files into `<DIR>`.
- The dry-run runtime arguments include `--grammar-file <DIR>/local-model-response.gbnf`.
- The dry-run JSON includes `contract_artifacts.schema_path`, `contract_artifacts.grammar_path`, `contract_artifacts.schema_fingerprint`, and `contract_artifacts.grammar_fingerprint`.
- Existing explicit `--grammar-path` behavior remains valid.
- README and roadmap document the new command path.
