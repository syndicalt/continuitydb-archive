# Local Model Artifact Directory Design

## Goal

Add `benchmark-local-model --artifact-dir` so operators can collect a coherent local Steward model trial bundle with one flag.

## Context

Real local model trials now have several useful artifacts: response contract files, per-case prompts, raw response files, response manifests, benchmark JSON reports, and baselines. Operators currently have to supply separate paths for contract, prompt, response, and report artifacts. A single bundle directory reduces setup mistakes and makes real small-model trials easier to archive and compare.

## Design

Add `--artifact-dir <path>` to `benchmark-local-model`.

When supplied:

- Write contract artifacts under `<artifact-dir>/contracts` unless `--contract-dir` is explicitly supplied.
- Write prompt artifacts under `<artifact-dir>/prompts` unless `--prompt-dir` is explicitly supplied.
- For real runs, write response artifacts under `<artifact-dir>/responses` unless `--response-dir` is explicitly supplied.
- Write the benchmark JSON report to `<artifact-dir>/benchmark-report.json` in addition to any explicit `--report-path`.

Dry-runs do not execute a model, so they write contracts, prompts, and the dry-run report, but no responses.

## Boundaries

This does not change benchmark semantics, baseline recording, model execution, or compatibility gates. Explicit artifact flags keep precedence for their individual artifact kind.

## Documentation

Add README and roadmap entries for CLI local-model benchmark artifact bundles.
