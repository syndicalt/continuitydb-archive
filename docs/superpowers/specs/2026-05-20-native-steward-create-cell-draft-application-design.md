# Native Steward CreateCellDraft Application Design

Add a deterministic native API path for accepted Steward `CreateCellDraft` proposals.

## Goal

An accepted `CreateCellDraft` proposal should become a real append-only StateCell only after deterministic policy accepts the proposal audit record.

## Semantics

- Rejected proposal audit records do not mutate committed state and return `None`.
- Accepted non-`CreateCellDraft` proposal records return `UnsupportedStewardProposalAction`.
- Accepted drafts preserve their proposed semantic anchors and text payload.
- The created StateCell represents policy-accepted draft content. Its evidence points to the Steward proposal citations, not to unsupported hidden model authority.
- The created StateCell uses:
  - the proposal's semantic anchors,
  - valid time starting at the application commit time,
  - project scope `continuitydb-steward`,
  - answerability question `what StateCell did the Steward draft?`,
  - derived evidence from every Steward proposal citation,
  - text payload from the draft,
  - token cost estimated from the draft text by whitespace token count.

This completes the current Steward proposal application coverage while preserving the rule that model output is accepted by deterministic policy before it becomes committed state.

## Tests

Add native API tests for:

- accepted draft appends a StateCell with proposed anchors and text payload,
- rejected draft returns `None` without appending,
- unsupported accepted action returns `UnsupportedStewardProposalAction`.
