# Native Steward Audit API Design

## Context

ContinuityDB now has a deterministic Steward proposal substrate, policy validation, proposal audit stores, and a kernel-backed proposal audit adapter. The native embeddable API can already ingest cells, checkout context, execute queries, record utility feedback, and inspect conflicts, but it does not expose Steward proposal audit operations directly.

That leaves embedders with two separate integration paths: use `ContinuityDb<K>` for database operations, then manually construct `StoredProposalLedger<KernelProposalStore<K>>` for Steward proposal audit. This is awkward because `KernelProposalStore<K>` owns the kernel, while `ContinuityDb<K>` already owns it.

## Design

Add a feature-gated bridge:

- `continuitydb-api` gets an optional `steward` feature that depends on `continuitydb-steward`.
- `continuitydb-steward` adds `BorrowedKernelProposalStore<'a, K>`, a `ProposalLedgerStore` adapter over `&'a mut K`. It reuses the same StateCell-backed audit format as `KernelProposalStore<K>` without taking ownership of the kernel.
- `ContinuityDb<K>` exposes Steward audit methods behind `#[cfg(feature = "steward")]`:
  - `record_steward_proposal(proposal, policy, decided_at) -> ProposalAuditRecord`
  - `steward_proposal_records() -> Vec<ProposalAuditRecord>`
  - `steward_proposal_record(proposal_id) -> Option<ProposalAuditRecord>`

All methods preserve the existing rule: the Steward emits proposals, deterministic policy decides, and the accepted or rejected proposal decision is stored as auditable StateCell evidence. The API does not execute an agent workflow and does not mutate operational truth beyond recording proposal audit cells.

## Error Handling

`ContinuityError` gains a feature-gated `Steward(#[from] StewardError)` variant. This keeps Steward errors typed when the feature is enabled and avoids adding a dependency to the default API build.

## Testing

Tests should prove:

- the API records a policy-evaluated Steward proposal into the backing kernel and can list/read it by proposal ID;
- rejected proposals are still audited with rejection reasons;
- `BorrowedKernelProposalStore` records to an existing kernel without consuming it.

## Roadmap Impact

This adds a Native API milestone and a Steward milestone. It tightens the embeddable boundary so applications can use one `ContinuityDb<K>` handle for deterministic database operations and Steward proposal audit.
