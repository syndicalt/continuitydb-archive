# Local Model Confidence Evaluation Design

## Goal

Expand the fixed local Steward model evaluation suite with a deterministic confidence-adjustment case.

## Scope

Add one default evaluation case that requires an `AdjustConfidence` proposal. This moves the local model benchmark beyond revision links and verification requests into belief-maintenance behavior, where the Steward proposes confidence changes from evidence while deterministic policy still validates the proposal.

## Behavior

The new case is named `confidence adjustment`.

It uses:

- cell ID: `StateCellId::from_u128(6)`
- evidence locator: `continuitydb://evaluation/confidence-evidence`
- expected proposed confidence: `0.42`
- required rationale term: `confidence`
- forbidden rationale term: `fully trusted`

The fixed suite should grow from six cases to seven cases. Benchmark JSON, baseline records, prompt artifacts, stability reports, failure reports, and evaluation-suite exports should all reflect the new case through the existing suite-driven paths.

## Architecture

Keep this as a suite-only extension. Do not change the Steward response schema, proposal policy, local executable runner, durable baseline shape, or CLI command arguments.

The case is added in `default_steward_evaluation_suite` after the unsupported-claim case and before multi-source citation preservation. Tests update complete local-model fixtures to include the confidence proposal and update case-count assertions from six to seven.

## Testing

Add RED tests proving:

- the default suite passes when a valid `AdjustConfidence` proposal is present;
- the default suite exposes the confidence case contract through public introspection;
- the CLI benchmark JSON reports seven passing cases for a complete local executable response;
- the CLI evaluation-suite export includes the confidence case and expected `adjust_confidence` action;
- prompt artifact generation includes the confidence prompt.

Full verification must include focused local-model tests, formatting, clippy, all-features tests, default tests, and `git diff --check`.
