# Native Steward LabelAnswerability Application Design

## Goal

Add deterministic native application for accepted Steward `LabelAnswerability` proposals.

This extends the proposal-to-state mutation path introduced for `MarkFrontier`: the Steward may propose that a stored `StateCell` can answer a revised set of questions, but only deterministic database code applies the accepted proposal by appending a successor cell.

## Scope

This slice applies only `StewardAction::LabelAnswerability` from an existing `ProposalAuditRecord`.

In scope:

- Require an accepted proposal decision before application.
- Return `Ok(None)` for rejected proposal records.
- Return `ContinuityError::UnsupportedStewardProposalAction` when an accepted record contains any non-`LabelAnswerability` action.
- Validate proposed questions through `Answerability::new`.
- Report missing target cells through `CellNotFound`.
- Append a successor `StateCell` at a caller-supplied deterministic commit time.
- Preserve the previous cell unchanged.

Out of scope:

- Merging proposed labels with prior labels.
- Applying `CreateCellDraft`, `LinkRevision`, `AdjustConfidence`, `RequestVerification`, or `MarkFrontier` through this method.
- Persisting revision graph edges as separate committed objects.
- Automatic application during proposal audit recording.

## API

Add a feature-gated native API method:

```rust
pub fn apply_accepted_label_answerability_proposal_at(
    &mut self,
    record: &ProposalAuditRecord,
    committed_at: DateTime<Utc>,
) -> Result<Option<StateCellId>, ContinuityError>
```

Behavior:

- Rejected records return `Ok(None)` with no mutation.
- Accepted `LabelAnswerability { cell_id, questions }` records look up the target cell, validate the questions, append a successor with replacement answerability, and return `Ok(Some(successor_id))`.
- Accepted unsupported actions return `UnsupportedStewardProposalAction`.
- Missing target cells return `CellNotFound`.

## Revision Helper

Add `revise_answerability(previous, answerability)` to `continuitydb-revision`.

The helper mirrors the activation and utility revision helpers: clone the prior cell, assign a new `StateCellId`, replace one field, and return `Supersedes` plus `Predecessor` revision links.

## Testing

Use TDD.

Tests must prove:

- Answerability revision creates a successor with replacement questions and revision links.
- Accepted `LabelAnswerability` records append a successor with replacement answerability while preserving the original.
- Rejected records no-op.
- Accepted unsupported actions error.
- Missing target cells error.

## Documentation

Update `README.md` and `docs/roadmap.md` with Native API and Steward milestones for accepted `LabelAnswerability` application.
