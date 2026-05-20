# Default Steward Evaluation Citation Preservation Design

## Problem

The fixed local Steward evaluation suite tested uncertainty, conflict classification, and unsupported claims, but it did not require preserving multiple evidence locators in one decision. A database steward must keep citations complete because partial citation loss weakens auditability and makes a frontier recommendation harder to verify.

## Goal

Add a default local-model evaluation case that requires a model to preserve two independent evidence citations when marking a release-status cell as frontier.

The case should flow through existing derived surfaces:

- default suite evaluation
- suite fingerprints
- CLI evaluation-suite export
- prompt artifact generation
- benchmark reports and baselines

## Case Contract

- Name: `multi-source citation preservation`
- Task: `Decide whether a release-status change should stay on the active frontier.`
- Evidence locators:
  - `continuitydb://evaluation/release-build-source`
  - `continuitydb://evaluation/release-incident-source`
- Expected action: `MarkFrontier` for `StateCellId::from_u128(3)`
- Required citations: both evidence locators
- Required rationale term: `frontier`

## Boundaries

- Do not change the response schema, grammar, or deterministic policy.
- Do not special-case CLI output; it should derive from the suite.
- Do not weaken existing fixed evaluation cases.

## Acceptance Criteria

- The default suite exposes four cases and the fourth case has two evidence snippets.
- A valid four-proposal response passes the default suite.
- CLI suite export reports four cases and includes both required citations for the fourth case.
- Prompt artifact generation writes a prompt for the fourth case containing both evidence locators.
- Benchmark recording reports four passing cases when the fixture emits all expected proposals.
