# Native Steward Frontier Audit API Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a feature-gated native API operation that runs subscribed frontier/watch stewardship and records proposal audit cells through `ContinuityDb<K>`.

**Architecture:** `continuitydb-api` owns the native operation. It accepts an existing `FrontierSubscriptionRunner<S>`, delegates subscription matching and proposal creation to it, evaluates each proposal with `ProposalPolicy`, and appends audit StateCells through `BorrowedKernelProposalStore`.

**Tech Stack:** Rust 2021, `continuitydb-api`, `continuitydb-steward`, `StorageKernel`.

---

## File Structure

- Modify `crates/continuitydb-api/src/lib.rs`: add `StewardFrontierAudit`, native method, imports, and feature-gated tests.
- Modify `README.md`: add the new native Steward frontier audit API to current scope.
- Modify `docs/roadmap.md`: add Native API and Steward milestones.
- Modify this plan file as tasks complete.

## Task 1: RED Tests

- [x] Add feature-gated test imports in `crates/continuitydb-api/src/lib.rs` for `FrontierSteward`, `FrontierSubscription`, `FrontierSubscriptionId`, `FrontierSubscriptionRunner`, `FrontierWatchEvent`, `FrontierWatchSignal`, and `MemoryFrontierSubscriptionStore`.
- [x] Add helper `test_frontier_runner(cell_id: StateCellId, signal: FrontierWatchSignal) -> Result<FrontierSubscriptionRunner<MemoryFrontierSubscriptionStore>, Box<dyn std::error::Error>>`:

```rust
#[cfg(feature = "steward")]
fn test_frontier_runner(
    cell_id: StateCellId,
    signal: FrontierWatchSignal,
) -> Result<FrontierSubscriptionRunner<MemoryFrontierSubscriptionStore>, Box<dyn std::error::Error>>
{
    let subscription = FrontierSubscription::new(
        FrontierSubscriptionId::new(),
        cell_id,
        vec![signal],
        "test://frontier-subscription",
        test_steward_time()?,
    )?;
    let mut store = MemoryFrontierSubscriptionStore::default();
    store.append_subscription(subscription)?;
    Ok(FrontierSubscriptionRunner::new(
        FrontierSteward::new(test_steward_identity()?),
        store,
    ))
}
```

- [x] Add test `api_audits_subscribed_frontier_watch_with_steward`:

```rust
#[cfg(feature = "steward")]
#[test]
fn api_audits_subscribed_frontier_watch_with_steward(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut db = ContinuityDb::new(MemoryKernel::default());
    let cell_id = StateCellId::new();
    let runner = test_frontier_runner(cell_id, FrontierWatchSignal::StaleEvidence)?;
    let event = FrontierWatchEvent::new(
        cell_id,
        FrontierWatchSignal::StaleEvidence,
        "test://frontier-stale",
        test_steward_time()?,
    );

    let audit = db.audit_frontier_watch_with_steward(
        &runner,
        vec![event],
        &ProposalPolicy::strict(),
        test_steward_time()?,
    )?;

    assert_eq!(audit.proposals.len(), 1);
    assert_eq!(audit.records.len(), 1);
    assert_eq!(audit.records[0].proposal().id(), audit.proposals[0].id());
    assert_eq!(audit.records[0].decision().outcome(), ProposalOutcome::Accepted);
    assert_eq!(
        audit.proposals[0].action(),
        &StewardAction::RequestVerification {
            cell_id: Some(cell_id),
            request: "Refresh stale evidence for frontier cell.".to_string(),
        }
    );
    assert_eq!(db.steward_proposal_records()?.len(), 1);
    Ok(())
}
```

- [x] Add test `api_frontier_watch_audit_ignores_unsubscribed_and_benign_events`:

```rust
#[cfg(feature = "steward")]
#[test]
fn api_frontier_watch_audit_ignores_unsubscribed_and_benign_events(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut db = ContinuityDb::new(MemoryKernel::default());
    let cell_id = StateCellId::new();
    let runner = test_frontier_runner(cell_id, FrontierWatchSignal::StaleEvidence)?;
    let events = vec![
        FrontierWatchEvent::new(
            StateCellId::new(),
            FrontierWatchSignal::StaleEvidence,
            "test://frontier-other-cell",
            test_steward_time()?,
        ),
        FrontierWatchEvent::new(
            cell_id,
            FrontierWatchSignal::Benign,
            "test://frontier-benign",
            test_steward_time()?,
        ),
    ];

    let audit = db.audit_frontier_watch_with_steward(
        &runner,
        events,
        &ProposalPolicy::strict(),
        test_steward_time()?,
    )?;

    assert!(audit.proposals.is_empty());
    assert!(audit.records.is_empty());
    assert!(db.steward_proposal_records()?.is_empty());
    Ok(())
}
```

- [x] Add test `api_frontier_watch_audit_deduplicates_multiple_matching_subscriptions`:

```rust
#[cfg(feature = "steward")]
#[test]
fn api_frontier_watch_audit_deduplicates_multiple_matching_subscriptions(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut db = ContinuityDb::new(MemoryKernel::default());
    let cell_id = StateCellId::new();
    let mut store = MemoryFrontierSubscriptionStore::default();
    for citation in ["test://frontier-subscription-1", "test://frontier-subscription-2"] {
        store.append_subscription(FrontierSubscription::new(
            FrontierSubscriptionId::new(),
            cell_id,
            vec![FrontierWatchSignal::HighImpactUncertainty],
            citation,
            test_steward_time()?,
        )?)?;
    }
    let runner = FrontierSubscriptionRunner::new(
        FrontierSteward::new(test_steward_identity()?),
        store,
    );

    let audit = db.audit_frontier_watch_with_steward(
        &runner,
        vec![FrontierWatchEvent::new(
            cell_id,
            FrontierWatchSignal::HighImpactUncertainty,
            "test://frontier-uncertain",
            test_steward_time()?,
        )],
        &ProposalPolicy::strict(),
        test_steward_time()?,
    )?;

    assert_eq!(audit.proposals.len(), 1);
    assert_eq!(audit.records.len(), 1);
    assert_eq!(db.steward_proposal_records()?.len(), 1);
    Ok(())
}
```

- [x] Run `cargo test -p continuitydb-api frontier_watch --features steward`.
- [x] Expected: FAIL because `StewardFrontierAudit` and `ContinuityDb::audit_frontier_watch_with_steward` do not exist.

## Task 2: Implementation

- [x] Add production imports behind `#[cfg(feature = "steward")]`:

```rust
use continuitydb_steward::{
    BorrowedKernelProposalStore, ConflictResolutionSteward, FrontierSubscriptionRunner,
    FrontierSubscriptionStore, FrontierWatchEvent, ProposalAuditRecord, ProposalId,
    ProposalPolicy, StewardError, StewardProposal, StoredProposalLedger,
};
```

- [x] Add the result type after `StewardConflictAudit`:

```rust
#[cfg(feature = "steward")]
pub struct StewardFrontierAudit {
    pub proposals: Vec<StewardProposal>,
    pub records: Vec<ProposalAuditRecord>,
}
```

- [x] Add method in `impl<K: StorageKernel> ContinuityDb<K>` near the Steward audit APIs:

```rust
#[cfg(feature = "steward")]
pub fn audit_frontier_watch_with_steward<S>(
    &mut self,
    runner: &FrontierSubscriptionRunner<S>,
    events: Vec<FrontierWatchEvent>,
    policy: &ProposalPolicy,
    decided_at: DateTime<Utc>,
) -> Result<StewardFrontierAudit, ContinuityError>
where
    S: FrontierSubscriptionStore,
{
    let proposals = runner.propose_subscribed(events, decided_at)?;
    let mut records = Vec::with_capacity(proposals.len());
    let store = BorrowedKernelProposalStore::new(&mut self.kernel);
    let mut ledger = StoredProposalLedger::new(store);

    for proposal in proposals.iter().cloned() {
        let decision = policy.evaluate(&proposal, decided_at);
        let record = ProposalAuditRecord::new(proposal.clone(), decision.clone())?;
        ledger.record(proposal, decision)?;
        records.push(record);
    }

    Ok(StewardFrontierAudit { proposals, records })
}
```

- [x] Run `cargo test -p continuitydb-api frontier_watch --features steward`.
- [x] Run `cargo test -p continuitydb-api --features steward`.

## Task 3: Docs, Gate, Commit

- [x] Add README current scope bullet: `Native feature-gated Steward frontier/watch audit API.`
- [x] Add Native API roadmap item after current #27: implemented `audit_frontier_watch_with_steward`.
- [x] Add Steward roadmap item after current #13: native API frontier/watch stewardship can emit and audit subscribed proposals.
- [x] Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

- [ ] Commit with `feat: add native steward frontier audit api`.

## Self-Review

- Spec coverage: The plan covers result type, native method, subscribed event behavior, unsubscribed/benign behavior, duplicate-subscription behavior, docs, and verification.
- Placeholder scan: No placeholders or vague implementation steps remain.
- Type consistency: Method and type names match the design document.
