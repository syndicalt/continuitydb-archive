# Local Model Evaluation Contract Introspection Design

## Problem

The default local-model Steward evaluation suite now contains fixed proposal-quality cases, but embedders cannot inspect those case contracts before running a model. That makes benchmark artifacts less transparent than the rest of the Steward surface: operators can see summary and per-case results after execution, but cannot programmatically inspect case names, inputs, expected actions, required citations, or rationale constraints in advance.

## Goal

Expose read-only public accessors for the feature-gated local-model evaluation suite, case contracts, and local-model input payloads so embedders can display or audit benchmark requirements before invoking a model.

## Non-Goals

- Do not make evaluation cases mutable after construction.
- Do not add serialization for case definitions in this slice.
- Do not change benchmark scoring, default case content, CLI output, or local model response contracts.

## API Shape

- `StewardEvaluationSuite::cases(&self) -> &[StewardEvaluationCase]`
- `StewardEvaluationSuite::len(&self) -> usize`
- `StewardEvaluationSuite::is_empty(&self) -> bool`
- `StewardEvaluationCase::name(&self) -> &str`
- `StewardEvaluationCase::input(&self) -> &LocalModelStewardInput`
- `StewardEvaluationCase::expected_actions(&self) -> &[StewardAction]`
- `StewardEvaluationCase::required_citations(&self) -> &[String]`
- `StewardEvaluationCase::required_rationale_terms(&self) -> &[String]`
- `StewardEvaluationCase::forbidden_rationale_terms(&self) -> &[String]`
- `LocalModelStewardInput::created_at(&self) -> DateTime<Utc>`
- `LocalModelStewardInput::task(&self) -> &str`
- `LocalModelStewardInput::evidence(&self) -> &[LocalModelEvidence]`

These methods return borrowed views or copy values. They preserve the current immutable-by-default evaluation contract and avoid exposing write access to internal vectors.

## Acceptance Criteria

- A feature-gated test proves the default suite exposes both current case contracts.
- The test verifies case names, input task text, evidence locators, expected actions, required citations, required rationale terms, and forbidden rationale terms.
- Existing evaluation behavior remains unchanged.
- README and roadmap mention public evaluation contract introspection.
