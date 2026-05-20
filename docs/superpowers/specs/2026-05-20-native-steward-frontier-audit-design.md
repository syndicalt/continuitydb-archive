# Native Steward Frontier Audit API Design

## Goal

Add a feature-gated native API operation that runs frontier/watch subscription stewardship and records policy-evaluated proposal audit records through the same `ContinuityDb<K>` backing kernel.

## Context

ContinuityDB already has deterministic frontier/watch pieces in `continuitydb-steward`:

- `FrontierSubscriptionStore` persists which cells and signals should trigger Steward work.
- `FrontierSubscriptionRunner<S>` filters incoming `FrontierWatchEvent`s through stored subscriptions.
- `FrontierSteward` emits `RequestVerification` and `MarkFrontier` proposals for subscribed events.
- `ContinuityDb::record_steward_proposal` can persist proposal audit StateCells through the database-owned kernel.

The missing native API layer forces embedders to run a subscription runner, evaluate policy, and record each proposal audit manually. The new method keeps that orchestration at the embeddable database boundary without turning ContinuityDB into an agent runtime.

## Selected Design

Add `StewardFrontierAudit` in `continuitydb-api` behind the existing `steward` feature:

```rust
pub struct StewardFrontierAudit {
    pub proposals: Vec<StewardProposal>,
    pub records: Vec<ProposalAuditRecord>,
}
```

Add this method on `ContinuityDb<K>` behind `#[cfg(feature = "steward")]`:

```rust
pub fn audit_frontier_watch_with_steward<S>(
    &mut self,
    runner: &FrontierSubscriptionRunner<S>,
    events: Vec<FrontierWatchEvent>,
    policy: &ProposalPolicy,
    decided_at: DateTime<Utc>,
) -> Result<StewardFrontierAudit, ContinuityError>
where
    S: FrontierSubscriptionStore;
```

The method delegates subscription matching and proposal creation to the supplied runner, then evaluates and records each proposal through `BorrowedKernelProposalStore`. It returns both emitted proposals and recorded audit records.

The method does not mutate operational truth beyond appending Steward proposal audit StateCells. `MarkFrontier` remains a proposal to change activation state; `RequestVerification` remains a proposal for external verification work.

## Error Behavior

- Subscription store errors and proposal construction errors map through `ContinuityError::Steward`.
- Benign, unsubscribed, empty, or non-matching events produce empty proposals and no audit records.
- Rejected proposals, if any, are still recorded because proposal audit is evidence about a policy decision.

## Testing

Add feature-gated API tests for:

- a subscribed stale-evidence event emits one `RequestVerification` proposal and records one accepted audit;
- an unsubscribed or benign event emits no proposals and records no audits;
- duplicate matching subscriptions for one event still produce one audited proposal because the runner already deduplicates per event.

## Roadmap Impact

This extends the Native API and Steward tracks by exposing frontier/watch stewardship as a first-class embeddable operation while preserving deterministic commit boundaries.
