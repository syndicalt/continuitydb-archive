# Targeted Verification Evaluation Design

## Goal

Add a default local Steward model evaluation case that tests targeted verification requests for a specific StateCell.

## Context

The fixed local-model suite already tests generic insufficient-evidence uncertainty, conflict classification, supersession classification, unsupported claims, confidence adjustment, multi-source citation preservation, and policy-rejection avoidance. It does not yet require a model to preserve a concrete `cell_id` when asking for verification.

## Design

Add one fixed case named `targeted verification request` to `default_steward_evaluation_suite`. The case represents a stale high-impact frontier cell where the correct model behavior is to emit a `RequestVerification` proposal tied to `StateCellId::from_u128(7)`.

The expected action is:

```rust
StewardAction::RequestVerification {
    cell_id: Some(StateCellId::from_u128(7)),
    request: "Refresh the stale high-impact frontier signal.".to_string(),
}
```

The case must require citation `continuitydb://evaluation/targeted-verification-evidence`, require rationale term `refresh`, and forbid rationale term `no target`. The case should be placed after `confidence adjustment` and before `multi-source citation preservation`, keeping the suite grouped as uncertainty, relationship classification, unsupported claims, confidence revision, targeted verification, citation preservation, and policy validation.

## Testing

Update Steward library tests to expect eight cases and verify the new case contract. Update CLI tests so benchmark output, failure reports, prompt artifacts, stability reports, and suite export all understand the new case count and ordering.

## Documentation

Add README current-scope and roadmap Steward milestone entries for the default targeted-verification evaluation case.
