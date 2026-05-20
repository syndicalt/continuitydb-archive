# Native Steward MarkFrontier Application Design

## Goal

Add the first deterministic application path for accepted Steward proposals: an accepted `MarkFrontier` proposal can be applied through the native API as an append-only successor `StateCell` whose activation state is `Frontier`.

This moves the Steward boundary from audit-only advice toward policy-mediated database maintenance while preserving the roadmap rule: models propose, deterministic database code commits.

## Scope

This slice applies only `StewardAction::MarkFrontier` from an existing `ProposalAuditRecord`.

In scope:

- Reject direct mutation of the prior `StateCell`.
- Require an accepted proposal decision before application.
- Return `Ok(None)` for rejected proposal records.
- Return a typed API error when an accepted record contains any non-`MarkFrontier` action.
- Report missing target cells through the existing `CellNotFound` error.
- Append the successor at a caller-supplied deterministic commit time.

Out of scope:

- Applying `CreateCellDraft`, `LinkRevision`, `AdjustConfidence`, `LabelAnswerability`, or `RequestVerification`.
- Persisting revision graph edges as separate database objects.
- Adding model inference or background execution.
- Applying proposals automatically during audit recording.

## API

Add a feature-gated method on `ContinuityDb<K>`:

```rust
pub fn apply_accepted_mark_frontier_proposal_at(
    &mut self,
    record: &ProposalAuditRecord,
    committed_at: DateTime<Utc>,
) -> Result<Option<StateCellId>, ContinuityError>
```

Behavior:

- If `record.decision().outcome()` is `Rejected`, return `Ok(None)` and do not append a cell.
- If the decision is accepted and the action is `MarkFrontier { cell_id }`, look up the target cell, create an append-only successor with a new `StateCellId`, set `activation = ActivationState::Frontier`, append it at `committed_at`, and return `Ok(Some(successor_id))`.
- If the decision is accepted and the action is not `MarkFrontier`, return `ContinuityError::UnsupportedStewardProposalAction`.

## Revision Helper

Add `revise_activation_state(previous, activation)` to `continuitydb-revision`.

The helper mirrors `revise_utility_feedback`: clone the prior cell, assign a new ID, update one field, and return a revision graph with `Supersedes` and `Predecessor` links. The native API persists the successor cell now; a future slice can persist revision graph edges natively.

## Testing

Use TDD.

Tests must prove:

- Accepted `MarkFrontier` proposals append a successor with `Frontier` activation while the original cell remains unchanged.
- Rejected `MarkFrontier` proposal records return `None` and append nothing.
- Accepted unsupported proposal actions return a typed error and append nothing.
- Accepted `MarkFrontier` proposals for missing cells return `CellNotFound`.

## Documentation

Update `README.md` current scope and `docs/roadmap.md` with Native API and Steward milestones for accepted `MarkFrontier` application.
