# Native Steward Conflict Audit API Design

## Goal

Add a feature-gated native API operation that analyzes stored StateCells for deterministic conflicts, converts the resulting recommendations into Steward proposals, records policy-evaluated proposal audit records through the same backing `StorageKernel`, and returns the full auditable result to the embedder.

## Context

ContinuityDB already has the separate pieces:

- `ContinuityDb::recommend_conflict_resolutions` materializes stored cells and returns a deterministic `ConflictResolutionScan`.
- `ConflictResolutionSteward` converts a scan into proposal objects and can record those proposals through a proposal ledger.
- `ContinuityDb::record_steward_proposal` can record an arbitrary proposal audit through the database-owned kernel.

The missing piece is a single native operation that keeps all of this inside the embeddable database API. Without it, applications must manually run conflict analysis, construct a Steward identity, call Steward conversion, and record audits one proposal at a time. That is correct but too easy to wire inconsistently.

## Selected Design

Add `StewardConflictAudit` in `continuitydb-api` behind the existing `steward` feature:

```rust
pub struct StewardConflictAudit {
    pub scan: ConflictResolutionScan,
    pub proposals: Vec<StewardProposal>,
    pub records: Vec<ProposalAuditRecord>,
}
```

Add this method on `ContinuityDb<K>` behind `#[cfg(feature = "steward")]`:

```rust
pub fn audit_conflict_resolutions_with_steward<I>(
    &mut self,
    cell_ids: I,
    steward: &ConflictResolutionSteward,
    policy: &ProposalPolicy,
    decided_at: DateTime<Utc>,
) -> Result<StewardConflictAudit, ContinuityError>
where
    I: IntoIterator<Item = StateCellId>;
```

The result type intentionally does not derive serde, equality, clone, or debug traits because the underlying revision scan types do not currently expose those traits. The method loads the requested cells once, builds the deterministic conflict-resolution scan, converts the borrowed scan into proposals through the supplied Steward, evaluates each proposal against the supplied policy, records each audit record through `BorrowedKernelProposalStore`, and returns the scan, proposals, and records.

The method does not apply revision links or mutate operational truth beyond appending Steward proposal audit StateCells. That preserves the database boundary: conflict analysis and Steward output remain proposals; deterministic policy decides audit acceptance; another explicit operation can later apply accepted mutations.

## Error Behavior

- Missing cell IDs keep using `ContinuityError::CellNotFound`.
- Steward proposal construction or audit storage failures use `ContinuityError::Steward`.
- Empty or singleton input returns an empty scan, empty proposals, and empty records.
- Rejected proposals, if any, are still recorded because proposal audit is evidence about the Steward decision, not only accepted truth.

## Testing

Add feature-gated API tests for:

- conflicting stored cells produce one accepted Steward audit record and one returned proposal;
- empty/singleton inputs produce no proposals or audit cells;
- missing cell IDs return the existing `CellNotFound` error before recording audit records.

## Roadmap Impact

This extends the Native API track and Steward track by making deterministic conflict-resolution stewardship a first-class embeddable operation without changing the datastore boundary or introducing an agent runtime.
