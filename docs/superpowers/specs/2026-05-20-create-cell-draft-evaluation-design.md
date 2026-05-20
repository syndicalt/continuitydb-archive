# Create Cell Draft Evaluation Design

## Goal

Add a default local Steward model evaluation case that tests `CreateCellDraft` proposals from new evidence.

## Context

The fixed local-model suite now evaluates uncertainty, conflict and supersession classification, unsupported claim boundaries, confidence adjustment, targeted verification, citation preservation, and policy-rejection avoidance. The Steward roadmap also allows StateCell creation from new evidence, but the fixed suite does not yet verify that a local model can propose a new draft cell without mutating committed truth.

## Design

Add one fixed case named `new evidence draft creation` to `default_steward_evaluation_suite`. The case represents new evidence about a benchmark result that does not map to an existing StateCell. The correct model behavior is to emit a `CreateCellDraft` proposal with a concrete semantic anchor and draft text payload.

The expected action is:

```rust
StewardAction::CreateCellDraft {
    anchors: vec![SemanticAnchor::new("project:continuitydb:benchmark-result")],
    payload_text: "ContinuityDB local Steward benchmark produced a new result requiring review.".to_string(),
}
```

The case must require citation `continuitydb://evaluation/new-benchmark-evidence`, require rationale term `draft`, and forbid rationale term `committed`. Place it after `targeted verification request` and before `multi-source citation preservation`, grouping creation and maintenance capabilities before citation-preservation and policy-validation cases.

## Testing

Update Steward library tests to expect nine cases and verify the new case contract. Update CLI benchmark fixtures, failure report counts, stability report counts, prompt artifacts, and evaluation-suite export assertions to use the new nine-case suite.

## Documentation

Add README current-scope and roadmap Steward milestone entries for the default create-cell-draft evaluation case.
