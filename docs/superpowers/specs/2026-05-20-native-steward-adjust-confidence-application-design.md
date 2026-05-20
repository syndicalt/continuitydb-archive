# Native Steward AdjustConfidence Application Design

## Goal

Add deterministic native application for accepted Steward `AdjustConfidence` proposals.

ContinuityDB currently models confidence on evidence entries, and checkout/ranking reads evidence confidence. This slice applies an accepted confidence proposal by appending a successor `StateCell` with replacement confidence on every evidence entry while preserving evidence source, citation, and trust metadata.

## Scope

This slice applies only `StewardAction::AdjustConfidence` from an existing `ProposalAuditRecord`.

In scope:

- Require an accepted proposal decision before application.
- Return `Ok(None)` for rejected proposal records.
- Return `ContinuityError::UnsupportedStewardProposalAction` when an accepted record contains any non-`AdjustConfidence` action.
- Validate the proposed scalar through `Confidence::new`.
- Report missing target cells through `CellNotFound`.
- Append a successor `StateCell` at a caller-supplied deterministic commit time.
- Preserve the previous cell unchanged.
- Preserve evidence source, citation, and trust metadata while replacing only evidence confidence.

Out of scope:

- Adding a separate cell-level confidence field.
- Per-evidence confidence adjustments.
- Confidence fusion across multiple evidence sources.
- Applying other Steward actions through this method.
- Persisting revision graph edges as separate committed objects.

## API

Add a feature-gated native API method:

```rust
pub fn apply_accepted_adjust_confidence_proposal_at(
    &mut self,
    record: &ProposalAuditRecord,
    committed_at: DateTime<Utc>,
) -> Result<Option<StateCellId>, ContinuityError>
```

Behavior:

- Rejected records return `Ok(None)` with no mutation.
- Accepted `AdjustConfidence { cell_id, proposed_confidence }` records validate the confidence, look up the target cell, append a successor with all evidence entries set to that confidence, and return `Ok(Some(successor_id))`.
- Accepted unsupported actions return `UnsupportedStewardProposalAction`.
- Missing target cells return `CellNotFound`.

## Revision Helper

Add `revise_evidence_confidence(previous, confidence)` to `continuitydb-revision`.

The helper clones the prior cell, assigns a new `StateCellId`, replaces each evidence entry's confidence, and returns `Supersedes` plus `Predecessor` revision links.

## Testing

Use TDD.

Tests must prove:

- Confidence revision creates a successor with updated confidence and revision links.
- Accepted `AdjustConfidence` records append a successor with replacement evidence confidence while preserving the original.
- Rejected records no-op.
- Accepted unsupported actions error.
- Missing target cells error.

## Documentation

Update `README.md` and `docs/roadmap.md` with Native API and Steward milestones for accepted `AdjustConfidence` application.
