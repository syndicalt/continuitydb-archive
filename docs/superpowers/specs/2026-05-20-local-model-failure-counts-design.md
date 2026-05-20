# Local Model Failure Counts Design

## Problem

Local Steward benchmark reports expose per-case failures and aggregate pass/fail totals, but CI and operators need deterministic aggregate counts by stable failure code. Without that, consumers must walk every case report to answer basic questions such as "did this run fail because of invalid model JSON or missing citations?"

## Constraints

- Use the stable snake-case failure codes already emitted in durable reports.
- Preserve per-case failure details.
- Keep ordering deterministic.
- Surface the counts through both the public evaluation report API and CLI benchmark JSON.

## Test

Add tests proving:

- `StewardEvaluationReport::failure_counts` counts repeated failure codes deterministically.
- CLI benchmark JSON includes a `failure_counts` object.
