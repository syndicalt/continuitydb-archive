# Checkout Revision Link Audit Traces Design

## Purpose

Direct `ContinuityDb::audit_cell` now enriches audit traces with native revision-link records. Deterministic checkout still builds selected-cell audit traces with the pure `audit(&StateCell)` helper, which leaves `AuditTrace.revision_links` empty even when selected cells participate in native supersession, conflict, predecessor, or derivation links.

This slice makes `CHECKOUT` output carry the same revision-link context for selected cells.

## Architecture

Keep `audit(&StateCell)` as the storage-free baseline. Update `checkout(kernel, request)` to enrich each selected cell's audit trace by querying the supplied `StorageKernel` for native revision links where the selected cell is source or target.

Use the same deterministic merge policy as direct API audit:

- source-side links first,
- target-side links second,
- self-links deduplicated.

This should not affect candidate lookup, scoring, token-budget packing, alternatives, uncertainty, or frontier recommendations.

## Tests

Tests must prove:

- Checkout-selected audit traces include source-side and target-side native revision links.
- Checkout-selected self-links are deduplicated.
- Existing metadata tests continue to pass with no revision links when no links exist.

## Roadmap Placement

Add Checkout milestone 14: checkout-selected revision-link audit enrichment.

