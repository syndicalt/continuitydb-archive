# File Workload Lookup Plan Regression Design

## Context

File workload measurement now records the lookup plan used by the measured checkout request. The durable baseline preserves indexed constraint labels, per-constraint candidate counts, final candidate counts, and full-scan fallback status, but baseline comparison only checks workload counts and elapsed thresholds.

## Decision

Extend workload baseline comparison so optional lookup-plan snapshots participate in deterministic regression reports.

Compare:

- lookup-plan presence,
- ordered indexed constraint names,
- final candidate count,
- full-scan status,
- per-constraint candidate counts for constraint names present in both baseline and current snapshots.

When both snapshots omit lookup-plan diagnostics, comparison remains passing. When only one snapshot has diagnostics, emit a presence regression instead of attempting partial comparison.

## Rationale

Persisted planner evidence is only useful for CI and operator workflows if a later measurement can detect changes in the plan shape. Candidate count growth and full-scan fallback are operationally meaningful regressions even when selected checkout counts remain unchanged.

## Non-goals

- Do not compare against missing legacy lookup plans as failures when both sides omit diagnostics.
- Do not infer planner quality from elapsed time alone.
- Do not add a new physical planner or storage engine in this slice.
