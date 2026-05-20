# Native Steward Accepted Proposal Dispatch Design

Add a unified native API entrypoint for applying an accepted Steward proposal audit record.

## Goal

Embedders should be able to pass a `ProposalAuditRecord` to one deterministic method and let ContinuityDB route the action to the correct application path.

## Semantics

- Rejected records return `None` through the same action-specific semantics.
- Accepted records dispatch by `StewardAction`:
  - `CreateCellDraft` -> `apply_accepted_create_cell_draft_proposal_at`
  - `LinkRevision` -> `apply_accepted_link_revision_proposal_at`
  - `AdjustConfidence` -> `apply_accepted_adjust_confidence_proposal_at`
  - `LabelAnswerability` -> `apply_accepted_label_answerability_proposal_at`
  - `MarkFrontier` -> `apply_accepted_mark_frontier_proposal_at`
  - `RequestVerification` -> `apply_accepted_request_verification_proposal_at`
- The dispatcher must not duplicate mutation logic. It delegates to action-specific methods so validation and committed representation remain single-sourced.
- Existing error behavior is preserved: missing cells, core validation failures, and storage failures are returned from the delegated application method.

This makes the policy-to-commit boundary easier for embedded applications while preserving deterministic database semantics.

## Tests

Add native API tests for:

- dispatcher applies an accepted create-cell draft,
- dispatcher applies an accepted frontier proposal,
- dispatcher returns `None` for a rejected record,
- dispatcher propagates missing-cell errors from delegated methods.
