# Native Steward RequestVerification Application Design

Add a deterministic native API path for accepted Steward `RequestVerification` proposals.

## Goal

An accepted verification request should become an append-only operational StateCell that embedders can checkout, audit, sync, and inspect like any other committed database state.

## Semantics

- Rejected proposal audit records do not mutate committed state and return `None`.
- Accepted non-`RequestVerification` proposal records return `UnsupportedStewardProposalAction`.
- Accepted cell-specific verification requests must reference an existing target StateCell. Missing targets return `CellNotFound`.
- Accepted targetless verification requests are allowed and create a general verification-work StateCell.
- The created StateCell represents the fact that deterministic policy accepted a verification task, not the truth of whatever the model suspects.
- The created StateCell uses:
  - anchors `continuitydb:steward:verification-request` and `continuitydb:steward:verification-request:<target-or-general>`,
  - valid time starting at the application commit time,
  - project scope `continuitydb-steward`,
  - answerability question `what verification did the Steward request?`,
  - derived evidence from the Steward proposal citations,
  - JSON payload containing the full `ProposalAuditRecord`,
  - token cost estimated from the request text by whitespace token count.

This preserves the boundary that models propose work while deterministic database policy decides what becomes committed state.

## Tests

Add native API tests for:

- accepted cell-specific request appends a verification StateCell and preserves the target dependency,
- accepted targetless request appends a general verification StateCell,
- rejected request returns `None` without appending,
- unsupported accepted action returns `UnsupportedStewardProposalAction`,
- missing target returns `CellNotFound`.
