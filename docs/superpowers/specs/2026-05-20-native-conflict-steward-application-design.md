# Native Conflict Steward Application Design

## Goal

Add a native embeddable API workflow that runs deterministic conflict-resolution stewardship, records proposal audits, and applies accepted proposal results through the typed Steward dispatcher.

## Context

ContinuityDB can already:

- detect deterministic StateCell conflicts,
- convert conflict-resolution recommendations into Steward proposals,
- record proposal audit StateCells,
- apply accepted proposals one record at a time,
- write accepted `LinkRevision` proposals as native revision-link records through the typed dispatcher.

The current conflict stewardship API stops after audit recording. Embedders must manually loop through returned audit records and call typed proposal application. That leaves the most important database-maintenance flow under-composed: "analyze these conflicting cells, record the policy decision, and commit the accepted database maintenance result."

## Architecture

Add a new feature-gated result type:

- `StewardConflictResolution`
  - `audit: StewardConflictAudit`
  - `applications: Vec<StewardApplicationResult>`

Add a new feature-gated native API method:

- `ContinuityDb::resolve_conflicts_with_steward_at(cell_ids, steward, policy, decided_at)`

The method should:

1. Use the existing `audit_conflict_resolutions_with_steward` method to preserve scan, proposals, records, and proposal-audit StateCells.
2. Iterate the resulting `ProposalAuditRecord`s in proposal order.
3. Apply each accepted record through `apply_accepted_steward_proposal_typed_at`.
4. Skip rejected records naturally because the typed dispatcher returns `None`.
5. Return the original audit plus ordered application results.

Accepted `LinkRevision` proposals therefore append native `RevisionLinkRecord`s. Accepted `RequestVerification` proposals still append operational work StateCells. The method does not change deterministic policy semantics or introduce model autonomy.

## Non-Goals

- This does not remove the legacy `apply_accepted_link_revision_proposal_at` API.
- This does not make audit plus application atomic across the storage kernel.
- This does not add retry/idempotency semantics for duplicate application attempts.
- This does not invoke a real local model.

## Verification

- A confidence-gap conflict produces one accepted `LinkRevision` proposal, one proposal-audit StateCell, and one native revision-link record.
- Empty or singleton conflict inputs return no proposals, no audit records, no applications, and no mutations.
- Missing input cell IDs fail before audit or application mutation.

## Roadmap Impact

- Native API: add a composed conflict-resolution stewardship workflow that records audit evidence and applies accepted maintenance results through the typed dispatcher.
- Steward: accepted conflict-resolution `LinkRevision` proposals can now be committed as native revision links through one embeddable API call.
