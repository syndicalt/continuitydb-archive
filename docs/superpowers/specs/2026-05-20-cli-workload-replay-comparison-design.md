# CLI Workload Replay Comparison Design

## Context

`replay-workload` can re-execute archived workload fixtures, but replay JSON alone still leaves CI consumers to manually compare replayed counts with the original `workload-report.json`. A replay command should be able to prove that a bundle still reproduces its deterministic workload and checkout result counts.

## Decision

Extend `replay-workload` with:

- `--compare-report`: read `workload-report.json` from the artifact directory and compare deterministic workload and checkout counts with the replay result.
- `--fail-on-mismatch`: exit non-zero when comparison reports mismatches.

The comparison intentionally ignores elapsed durations. It compares workload shape, total token cost, matched/selected/alternative checkout counts, selected frontier count, and selected token count. Output includes `replay_comparison` with `passed`, report path, and stable mismatch objects.

## Non-goals

- Do not compare elapsed timings in replay comparison.
- Do not compare baseline-regression metadata.
- Do not mutate baselines or artifact files.
