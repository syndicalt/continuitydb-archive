# API Batch Conflict Analysis Design

## Goal

Expose deterministic batch conflict analysis through the native embeddable API. Applications should be able to pass an ordered set of stored `StateCellId` values and receive pairwise conflict scans or conflict-resolution recommendations without manually loading cells.

## Scope

This slice is read-only. It does not accept recommendations, persist revision links, retire cells, or mutate committed truth. It only exposes existing deterministic batch revision analysis through `continuitydb-api`.

## Architecture

`continuitydb-revision` already provides:

- `scan_cell_conflicts(&[StateCell]) -> CellConflictScan`
- `recommend_conflict_resolutions(&[StateCell]) -> ConflictResolutionScan`

`ContinuityDb<K>` will add:

- `detect_conflicts(cell_ids: impl IntoIterator<Item = StateCellId>)`
- `recommend_conflict_resolutions(cell_ids: impl IntoIterator<Item = StateCellId>)`

Both methods resolve cells by ID in caller-provided order using the existing private lookup helper. The resolved cells are passed to the lower-level deterministic batch functions.

## Semantics

Input order is preserved and defines deterministic pair traversal. Missing cells return `ContinuityError::CellNotFound` for the first missing ID encountered. Empty and one-cell inputs are valid and return empty scans.

The operation remains non-mutating. Returned revision graphs are proposals or explanatory metadata only; they are not persisted by this API.

## Testing

Tests must prove:

- Batch conflict detection returns only the conflicting pair from a mixed input set.
- Batch resolution returns deterministic confidence-gap recommendations in input-pair order.
- Empty and one-cell inputs return empty scans.
- Missing IDs return `CellNotFound`.

## Roadmap Impact

This adds a Native API milestone for batch revision analysis. It prepares checkout, Steward, and future query-language layers to analyze conflict frontiers over continuity slices without bypassing the API boundary.
