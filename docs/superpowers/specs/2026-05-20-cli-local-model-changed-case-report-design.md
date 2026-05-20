# CLI Local Model Changed-Case Report Design

## Problem

Local Steward benchmark regression JSON now includes detailed changed-case summaries, but CI consumers that only need drift routing still have to archive and parse the full benchmark report.

## Design

- Add `benchmark-local-model --changed-case-report-path <path>`.
- When a baseline comparison exists, write a compact JSON report with stable format metadata, candidate identity, baseline path, comparison counts, and `changed_case_summaries`.
- Include `changed_case_report_path` in benchmark JSON so artifacts are discoverable from stdout or the full report.
- Do not change regression gating semantics or baseline recording behavior.

## Test

- Add a feature-gated CLI test that records a passing baseline, reruns with a changed passing response, requests `--compare-baseline --changed-case-report-path`, and asserts the compact report contains total, reason-specific counts, and changed-case summaries.
