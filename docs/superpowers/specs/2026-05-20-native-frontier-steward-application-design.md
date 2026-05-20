# Native Frontier Steward Application Design

## Goal

Add a native embeddable API workflow that runs subscribed frontier/watch stewardship, records proposal audits, and applies accepted proposal results through the typed Steward dispatcher.

## Context

ContinuityDB can already audit frontier/watch events through subscribed deterministic Steward proposals. Accepted frontier proposals can also be applied individually:

- `RequestVerification` appends operational verification-work StateCells.
- `MarkFrontier` appends successor StateCells with `Frontier` activation.

The current native frontier/watch workflow stops after audit recording. Embedders still need to loop through returned records and dispatch applications themselves. A database-maintenance workflow should expose a direct operation for "watch these events, audit the policy decision, and commit accepted maintenance results."

## Architecture

Add a feature-gated result type:

- `StewardFrontierResolution`
  - `audit: StewardFrontierAudit`
  - `applications: Vec<StewardApplicationResult>`

Add a feature-gated native API method:

- `ContinuityDb::resolve_frontier_watch_with_steward_at(runner, events, policy, decided_at)`

The method should:

1. Call `audit_frontier_watch_with_steward` to preserve subscription filtering, proposal generation, policy evaluation, and proposal-audit StateCells.
2. Iterate returned audit records in proposal order.
3. Apply each accepted record through `apply_accepted_steward_proposal_typed_at`.
4. Collect `Some` application results and skip rejected records.
5. Return the audit plus ordered application results.

This keeps the model-as-proposer boundary intact. The Steward proposes; deterministic policy accepts or rejects; the typed dispatcher commits accepted database maintenance.

## Non-Goals

- This does not change frontier subscription matching.
- This does not add atomic multi-record transactions across audit and application.
- This does not invoke a real local model.
- This does not remove the existing audit-only API.

## Verification

- A subscribed stale-evidence event for an existing StateCell records one proposal audit and applies one verification-work StateCell.
- A subscribed high-impact-uncertainty event records one proposal audit and applies one frontier successor StateCell.
- Unsubscribed and benign events produce no records or applications.

## Roadmap Impact

- Native API: add a composed frontier/watch Steward audit-and-application workflow.
- Steward: subscribed frontier/watch events can now commit accepted verification and frontier-maintenance results through one embeddable API call.
