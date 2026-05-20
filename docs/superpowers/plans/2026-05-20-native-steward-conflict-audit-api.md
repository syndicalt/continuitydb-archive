# Native Steward Conflict Audit API Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a feature-gated native API operation that runs deterministic conflict-resolution stewardship and records proposal audit cells through `ContinuityDb<K>`.

**Architecture:** `continuitydb-api` owns the native operation. It reuses existing stored-cell lookup, `continuitydb-revision::recommend_conflict_resolutions`, `ConflictResolutionSteward::propose`, `ProposalPolicy`, and `BorrowedKernelProposalStore`; it never applies revision truth.

**Tech Stack:** Rust 2021, `continuitydb-api`, `continuitydb-steward`, `continuitydb-revision`, `StorageKernel`.

---

## File Structure

- Modify `crates/continuitydb-api/src/lib.rs`: add `StewardConflictAudit`, native method, imports, and feature-gated tests.
- Modify `README.md`: add the new native Steward conflict audit API to current scope.
- Modify `docs/roadmap.md`: add Native API and Steward milestones.
- Modify this plan file as tasks complete.

## Task 1: RED Tests

- [ ] Add feature-gated imports in `crates/continuitydb-api/src/lib.rs` tests for `ConflictResolutionSteward`, `ProposalOutcome`, `RevisionLinkKind`, and `StewardAction`.
- [ ] Add helper `test_conflict_steward() -> Result<ConflictResolutionSteward, Box<dyn std::error::Error>>` using `StewardIdentity::new("native-api-conflict-steward", "0.1.0", "deterministic-policy")`.
- [ ] Add test `api_audits_conflict_resolutions_with_steward`:

```rust
#[cfg(feature = "steward")]
#[test]
fn api_audits_conflict_resolutions_with_steward() -> Result<(), Box<dyn std::error::Error>> {
    let committed_at = test_steward_time()?;
    let mut db = ContinuityDb::new(MemoryKernel::default());
    let low = sample_cell_with_payload_day_and_confidence(
        "project:continuitydb:steward-conflict",
        "release is blocked",
        20,
        0.55,
        12,
    )?;
    let high = sample_cell_with_payload_day_and_confidence(
        "project:continuitydb:steward-conflict",
        "release is ready",
        21,
        0.95,
        12,
    )?;
    let low_id = db.ingest_cell_at(low, committed_at)?;
    let high_id = db.ingest_cell_at(high, committed_at)?;
    let steward = test_conflict_steward()?;

    let audit = db.audit_conflict_resolutions_with_steward(
        [low_id, high_id],
        &steward,
        &ProposalPolicy::strict(),
        committed_at,
    )?;

    assert_eq!(audit.scan.recommendations.len(), 1);
    assert_eq!(audit.proposals.len(), 1);
    assert_eq!(audit.records.len(), 1);
    assert_eq!(audit.records[0].proposal().id(), audit.proposals[0].id());
    assert_eq!(audit.records[0].decision().outcome(), ProposalOutcome::Accepted);
    assert_eq!(
        audit.proposals[0].action(),
        &StewardAction::LinkRevision {
            source: high_id,
            kind: RevisionLinkKind::Supersedes,
            target: low_id,
        }
    );
    assert_eq!(db.steward_proposal_records()?.len(), 1);
    Ok(())
}
```

- [ ] Add test `api_steward_conflict_audit_allows_empty_and_singleton_inputs`:

```rust
#[cfg(feature = "steward")]
#[test]
fn api_steward_conflict_audit_allows_empty_and_singleton_inputs(
) -> Result<(), Box<dyn std::error::Error>> {
    let committed_at = test_steward_time()?;
    let mut db = ContinuityDb::new(MemoryKernel::default());
    let cell = sample_cell_with_payload_day_and_confidence(
        "project:continuitydb:steward-singleton",
        "release is ready",
        20,
        0.95,
        12,
    )?;
    let cell_id = db.ingest_cell_at(cell, committed_at)?;
    let steward = test_conflict_steward()?;

    let empty = db.audit_conflict_resolutions_with_steward(
        [],
        &steward,
        &ProposalPolicy::strict(),
        committed_at,
    )?;
    let singleton = db.audit_conflict_resolutions_with_steward(
        [cell_id],
        &steward,
        &ProposalPolicy::strict(),
        committed_at,
    )?;

    assert!(empty.scan.recommendations.is_empty());
    assert!(empty.proposals.is_empty());
    assert!(empty.records.is_empty());
    assert!(singleton.scan.recommendations.is_empty());
    assert!(singleton.proposals.is_empty());
    assert!(singleton.records.is_empty());
    assert!(db.steward_proposal_records()?.is_empty());
    Ok(())
}
```

- [ ] Add test `api_steward_conflict_audit_reports_missing_id_without_audit`:

```rust
#[cfg(feature = "steward")]
#[test]
fn api_steward_conflict_audit_reports_missing_id_without_audit(
) -> Result<(), Box<dyn std::error::Error>> {
    let committed_at = test_steward_time()?;
    let mut db = ContinuityDb::new(MemoryKernel::default());
    let cell = sample_cell_with_payload_day_and_confidence(
        "project:continuitydb:steward-missing",
        "release is ready",
        20,
        0.95,
        12,
    )?;
    let cell_id = db.ingest_cell_at(cell, committed_at)?;
    let missing_id = StateCellId::new();
    let steward = test_conflict_steward()?;

    let result = db.audit_conflict_resolutions_with_steward(
        [cell_id, missing_id],
        &steward,
        &ProposalPolicy::strict(),
        committed_at,
    );

    assert!(matches!(
        result,
        Err(ContinuityError::CellNotFound { cell_id }) if cell_id == missing_id
    ));
    assert!(db.steward_proposal_records()?.is_empty());
    Ok(())
}
```

- [ ] Run `cargo test -p continuitydb-api steward_conflict --features steward`.
- [ ] Expected: FAIL because `StewardConflictAudit` and `ContinuityDb::audit_conflict_resolutions_with_steward` do not exist.

## Task 2: Implementation

- [ ] Add production imports behind `#[cfg(feature = "steward")]`:

```rust
use continuitydb_steward::{
    BorrowedKernelProposalStore, ConflictResolutionSteward, ProposalAuditRecord, ProposalId,
    ProposalPolicy, StewardError, StewardProposal, StoredProposalLedger,
};
```

- [ ] Add the result type after `CommitExportBatch`:

```rust
#[cfg(feature = "steward")]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StewardConflictAudit {
    pub scan: ConflictResolutionScan,
    pub proposals: Vec<StewardProposal>,
    pub records: Vec<ProposalAuditRecord>,
}
```

- [ ] Add method in `impl<K: StorageKernel> ContinuityDb<K>` near conflict APIs:

```rust
#[cfg(feature = "steward")]
pub fn audit_conflict_resolutions_with_steward<I>(
    &mut self,
    cell_ids: I,
    steward: &ConflictResolutionSteward,
    policy: &ProposalPolicy,
    decided_at: DateTime<Utc>,
) -> Result<StewardConflictAudit, ContinuityError>
where
    I: IntoIterator<Item = StateCellId>,
{
    let cells = self.lookup_cells_in_order(cell_ids)?;
    let scan = recommend_conflict_resolutions(&cells);
    let proposals = steward.propose(scan.clone(), decided_at)?;
    let mut records = Vec::with_capacity(proposals.len());
    let store = BorrowedKernelProposalStore::new(&mut self.kernel);
    let mut ledger = StoredProposalLedger::new(store);

    for proposal in proposals.iter().cloned() {
        let decision = policy.evaluate(&proposal, decided_at);
        let record = ProposalAuditRecord::new(proposal.clone(), decision.clone())?;
        ledger.record(proposal, decision)?;
        records.push(record);
    }

    Ok(StewardConflictAudit {
        scan,
        proposals,
        records,
    })
}
```

- [ ] Run `cargo test -p continuitydb-api steward_conflict --features steward`.
- [ ] Run `cargo test -p continuitydb-api --features steward`.

## Task 3: Docs, Gate, Commit

- [ ] Add README current scope bullet: `Native feature-gated Steward conflict-resolution audit API.`
- [ ] Add Native API roadmap item after current #26: implemented `audit_conflict_resolutions_with_steward`.
- [ ] Add Steward roadmap item after current #12: native API conflict-resolution stewardship can emit and audit proposal decisions.
- [ ] Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace
git diff --check
```

- [ ] Commit with `feat: add native steward conflict audit api`.

## Self-Review

- Spec coverage: The plan covers the result type, native method, accepted audit behavior, empty/singleton behavior, missing-ID behavior, docs, and verification.
- Placeholder scan: No placeholders or vague implementation steps remain.
- Type consistency: Method and type names match the design document.
