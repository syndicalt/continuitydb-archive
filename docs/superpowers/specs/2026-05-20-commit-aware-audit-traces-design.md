# Commit-Aware Audit Traces Design

## Goal

Expose the database commit boundary in audit traces. A selected or directly audited `StateCell` should explain which `CommitId` wrote it, alongside citations, structured evidence, and dependency metadata.

## Scope

This slice only adds commit identity to `AuditTrace`. It does not add commit records, commit summaries, author/source metadata, write manifests, or recovery logs.

## Architecture

`AuditTrace` gains:

```rust
pub commit_id: CommitId
```

The existing `audit(cell)` function copies `cell.commit_id` into the trace. Checkout already builds `audit_traces` by mapping selected cells through `audit`, so checkout output receives commit-aware audit traces without extra logic.

## Semantics

Every audit trace carries the commit ID from its source cell. Legacy or uncommitted cells naturally report the nil commit ID because `StateCell.commit_id` defaults to nil.

The commit ID is informational audit metadata. It does not change checkout ranking, filtering, or conflict/revision behavior.

## Testing

Tests must prove:

- Direct `audit(cell)` includes the cell commit ID.
- Checkout audit traces include commit IDs for selected cells.
- Existing audit/evidence/dependency behavior remains unchanged.

## Roadmap Impact

This adds a checkout/audit milestone that connects transaction identity to human- and agent-readable explanations. It prepares future commit records and proof APIs to answer "which database commit produced this operational truth?"
