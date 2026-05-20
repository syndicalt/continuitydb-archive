# CLI Local Model Failure Report Artifact Design

## Goal

When `benchmark-local-model --fail-on-failed-cases` rejects a model candidate, operators should still get a structured JSON artifact explaining the failed cases without polluting benchmark baselines.

## Scope

Add `--failure-report-path <PATH>` to `benchmark-local-model`. The first use is for fixed evaluation failures: when `--fail-on-failed-cases` rejects the current run, the CLI writes the normal benchmark JSON report to this path before returning a non-zero error. It does not append a baseline.

## Behavior

When the flag is absent, behavior remains unchanged.

When `--failure-report-path` is present on a dry-run, the JSON output includes the configured path and does not create the file.

When `--failure-report-path` is present with `--fail-on-failed-cases` and the evaluation fails:

- Run the benchmark once.
- Build the current benchmark baseline in memory.
- Write the normal benchmark JSON output to `failure_report_path`.
- Return a non-zero error.
- Do not create or append the benchmark baseline file.

When the evaluation passes, no failure report is written and the baseline path is recorded normally.

## Architecture

Keep the reporting shape identical to `local_model_benchmark_json` so operators can parse one schema for both successful and rejected runs. The failure artifact should include evaluation details, fingerprints, runtime metadata, prompt and contract artifact metadata, and any stability report that was requested.

The file write happens only after the current in-memory baseline exists and before returning the failed-case gate error. This preserves deterministic gate behavior and avoids baseline mutation.

## Testing

Add RED tests proving:

- Dry-run reports the configured failure report path and does not create the file.
- A failed fixed evaluation with `--failure-report-path` writes JSON with failed case details and does not create the baseline.
- A passing fixed evaluation with `--failure-report-path` records the baseline and does not create the failure report.

Full verification must include focused tests, formatting, clippy, all-features tests, default tests, and `git diff --check`.
