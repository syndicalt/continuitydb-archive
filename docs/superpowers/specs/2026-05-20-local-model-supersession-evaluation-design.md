# Local Model Supersession Evaluation Design

## Goal

Expand the fixed local Steward model evaluation suite with a deterministic supersession classification case.

## Scope

Add one default evaluation case that requires a `LinkRevision` proposal with `kind = Supersedes`. This complements the existing `ConflictsWith` case so local model candidates are evaluated on the distinction between contradictory evidence and newer evidence that replaces older state.

## Behavior

The new case is named `supersession classification`.

It uses:

- source cell ID: `StateCellId::from_u128(4)`
- target cell ID: `StateCellId::from_u128(5)`
- evidence locator: `continuitydb://evaluation/supersession-evidence`
- required rationale term: `supersedes`
- forbidden rationale term: `conflicts with`

The fixed suite should grow from five cases to six cases. Benchmark JSON, baseline records, prompt artifacts, stability reports, failure reports, and evaluation-suite exports should all reflect the new case through the existing suite-driven paths.

## Architecture

Keep this as a suite-only extension. Do not change the Steward response schema, policy layer, local executable runner, baseline format, or CLI command shape.

The case is added in `default_steward_evaluation_suite`. Tests update the existing passing fixture responses to include the supersession proposal, and update case-count assertions from five to six.

## Testing

Add RED tests proving:

- the default suite passes when a valid `Supersedes` revision-link proposal is present;
- the default suite exposes the supersession case contract through public introspection;
- the CLI benchmark JSON reports six passing cases for a complete local executable response;
- the CLI evaluation-suite export includes the supersession case and expected `supersedes` kind;
- prompt artifact generation includes the supersession prompt.

Full verification must include focused local-model tests, formatting, clippy, all-features tests, default tests, and `git diff --check`.
