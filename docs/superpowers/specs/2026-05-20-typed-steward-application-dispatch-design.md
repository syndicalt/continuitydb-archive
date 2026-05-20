# Typed Steward Application Dispatch Design

## Purpose

The native API now has two ways to apply accepted Steward `LinkRevision` proposals:

- the existing `apply_accepted_link_revision_proposal_at`, which preserves compatibility by appending an operational link `StateCell`,
- the newer `apply_accepted_link_revision_record_proposal_at`, which writes a native `RevisionLinkRecord`.

The unified `apply_accepted_steward_proposal_at` dispatcher still returns `Option<StateCellId>`, so it cannot route `LinkRevision` through the native revision-link record path without breaking its return type. This slice adds an additive typed dispatcher that can represent both committed StateCell revisions and native revision-link records.

## Architecture

Add a feature-gated API result enum:

- `StewardApplicationResult::StateCell(StateCellId)` for accepted actions that append a StateCell,
- `StewardApplicationResult::RevisionLink(RevisionLinkRecord)` for accepted `LinkRevision` actions that append a native revision-link record.

Add a new feature-gated method:

- `apply_accepted_steward_proposal_typed_at(record, committed_at) -> Option<StewardApplicationResult>`

Rejected proposals return `Ok(None)`. Supported StateCell-producing actions delegate to their current action-specific methods. `LinkRevision` delegates to `apply_accepted_link_revision_record_proposal_at`, so the typed dispatcher writes native revision-link records and does not add an operational link StateCell.

The existing `apply_accepted_steward_proposal_at` remains unchanged for callers that depend on `Option<StateCellId>`.

## Tests

Tests must prove:

- Typed dispatch of accepted `CreateCellDraft` returns `StateCell` and appends the expected cell.
- Typed dispatch of accepted `LinkRevision` returns `RevisionLink`, persists a native revision-link record, and does not append an operational link StateCell.
- Typed dispatch of rejected proposals returns `None` and performs no mutation.
- Missing endpoints in typed `LinkRevision` dispatch still return `CellNotFound`.

## Roadmap Placement

Add Native API milestone 37: typed accepted Steward proposal application result.

