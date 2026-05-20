# Native Steward LinkRevision Application Design

Add a deterministic native API path for accepted Steward `LinkRevision` proposals.

## Goal

An accepted `LinkRevision` proposal should become committed, auditable database state without allowing model output to mutate hidden revision structures directly.

## Semantics

- Rejected proposal audit records do not mutate committed state and return `None`.
- Accepted non-`LinkRevision` proposal records return `UnsupportedStewardProposalAction`.
- Accepted `LinkRevision` proposals must reference existing source and target StateCells. Missing endpoints return `CellNotFound`.
- The committed StateCell represents the accepted revision-link assertion, not an unlogged in-memory graph mutation.
- The created StateCell uses:
  - anchors `continuitydb:steward:revision-link` and `continuitydb:steward:revision-link:<source>:<kind>:<target>`,
  - valid time starting at the application commit time,
  - project scope `continuitydb-steward`,
  - answerability question `what revision link did the Steward propose?`,
  - derived evidence from the Steward proposal citations,
  - JSON payload containing the full `ProposalAuditRecord`,
  - dependencies to the source and target StateCells.

This is intentionally conservative. A future storage kernel can add native revision-link records; this slice commits the link assertion as queryable operational state today.

## Tests

Add native API tests for:

- accepted link proposal appends an operational revision-link StateCell,
- rejected link proposal returns `None` without appending,
- unsupported accepted action returns `UnsupportedStewardProposalAction`,
- missing source returns `CellNotFound`,
- missing target returns `CellNotFound`.
