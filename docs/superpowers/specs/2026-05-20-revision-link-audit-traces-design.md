# Revision Link Audit Traces Design

## Purpose

ContinuityDB can now persist native `RevisionLinkRecord` values and apply accepted Steward `LinkRevision` proposals into that native record store. Direct cell audit still only reports the audited StateCell's citations, evidence, dependencies, and commit ID. That leaves supersession, conflict, predecessor, and derivation links outside the primary audit explanation.

This slice makes native revision links visible in direct API audit traces.

## Architecture

Extend `continuitydb-checkout::AuditTrace` with:

- `revision_links: Vec<RevisionLinkRecord>`

The base `checkout::audit(&StateCell)` function should keep returning an empty revision-link list because it has only a cell value, not a storage kernel. The native API `ContinuityDb::audit_cell` owns kernel access, so it should enrich the trace by listing revision links where the audited cell is either the source or target.

Use two existing storage-kernel lookups:

- `RevisionLinkLookup { source: Some(cell_id), .. }`
- `RevisionLinkLookup { target: Some(cell_id), .. }`

Merge the two lists in deterministic order, preserving source matches first and target matches second while deduplicating a self-link that appears in both lists.

## Tests

Tests must prove:

- `audit_cell` returns source-side and target-side native revision links for the audited cell.
- Self-links do not appear twice in one audit trace.
- Existing missing-cell audit behavior remains unchanged.

## Roadmap Placement

Add Checkout milestone 13 and Native API milestone 38 for revision-link-aware audit traces.

