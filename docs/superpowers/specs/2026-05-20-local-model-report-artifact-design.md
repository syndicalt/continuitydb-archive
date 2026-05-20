# Local Model Report Artifact Design

## Goal

Add a `benchmark-local-model --report-path` option that writes the successful benchmark JSON payload to a durable artifact file.

## Context

`benchmark-local-model` already prints structured JSON to stdout, writes failure reports for pre-recording gates, and records durable JSONL baselines. Real local Steward model trials need a stable single-run report artifact even when the run succeeds or is a dry-run preflight. Shell redirection can capture stdout, but a first-class option makes CI and reproducible trial scripts deterministic and mirrors the existing failure-report path.

## Design

Add a feature-gated `--report-path <path>` option to `benchmark-local-model`. When command execution succeeds, write the same pretty JSON object printed to stdout to the report path. This applies to both real benchmark runs and dry runs.

The option does not change baseline recording, failure gate behavior, evaluation semantics, or model execution. Gate failures continue to use `--failure-report-path`, because failures return an error before the normal success payload is available.

## Testing

Add CLI tests for:

- A dry run with `--report-path` writes the preflight JSON and does not create a baseline.
- A passing real run with `--report-path` writes the benchmark JSON, records the baseline, and the artifact contains the same candidate and pass-count fields as stdout.

## Documentation

Add README and roadmap entries for the CLI local-model benchmark report artifact path.
