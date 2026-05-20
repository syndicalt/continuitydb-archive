# Default Steward Evaluation Unsupported Claim Design

## Problem

The fixed local Steward evaluation suite covered insufficient evidence and conflict classification, but it did not directly test whether a model avoids inventing certainty beyond source evidence. A database steward must preserve evidence boundaries because unsupported operational claims can become false committed truth if accepted by policy.

## Goal

Add a default local-model evaluation case that checks unsupported-claim handling. The case should require a `RequestVerification` proposal when release evidence does not support a shipped deployment claim.

The case must be included automatically in:

- `default_steward_evaluation_suite()`
- evaluation suite fingerprints
- CLI `local-model-evaluation-suite` JSON
- prompt artifact generation
- benchmark reports and baselines

## Case Contract

- Name: `unsupported claim boundary`
- Task: `Check whether release evidence supports a shipped deployment claim.`
- Evidence locator: `continuitydb://evaluation/unsupported-release-claim`
- Expected action: `RequestVerification` with no target cell and request text `Verify deployment status before treating the release as shipped.`
- Required citation: the unsupported-claim evidence locator
- Required rationale term: `unsupported`
- Forbidden rationale term: `deployed to all customers`

## Boundaries

- Do not add nondeterministic model behavior.
- Do not change the response schema or grammar.
- Do not change deterministic policy acceptance semantics.
- Do not weaken existing evaluation cases.

## Acceptance Criteria

- The default suite exposes three cases and the third case has the unsupported-claim contract.
- A valid three-proposal response passes the default suite.
- CLI suite export reports three cases and includes the unsupported-claim contract.
- Prompt artifact generation writes a prompt for the unsupported-claim case.
- Benchmark recording treats all three default cases as passing when the fixture emits all expected proposals.
