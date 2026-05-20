# API Conflict Analysis Design

## Goal

Expose deterministic StateCell conflict analysis through the native embeddable API. Applications should be able to ask whether two stored cells conflict and what deterministic resolution policy recommends without manually loading cells or calling lower-level revision helpers.

## Scope

This slice is read-only. It does not accept a recommendation, mutate cells, persist revision links, or retire superseded cells. Those are later operations once durable revision-link representation is explicit.

## Architecture

`continuitydb-api` already depends on `continuitydb-revision`. `ContinuityDb<K>` will add:

- `detect_conflict(left_id, right_id)`
- `recommend_conflict_resolution(left_id, right_id)`

Both methods resolve cells by immutable ID through the existing private lookup helper. Missing IDs return `ContinuityError::CellNotFound`. Successful lookups delegate to `detect_cell_conflict` and `recommend_conflict_resolution`.

## Semantics

No conflict is represented as `Ok(None)`. A conflict or recommendation is represented as `Ok(Some(...))`. The methods preserve deterministic lower-level behavior: same-anchor, overlapping-valid-time, different-payload conflicts; confidence-gap, latest-valid-time, or human-review recommendations.

The API does not hide which cell was missing. If the left ID is missing, it reports that ID before looking up the right side. If the left exists and the right is missing, it reports the right ID.

## Testing

Tests must prove:

- `detect_conflict` returns conflict metadata for two stored conflicting cells.
- `recommend_conflict_resolution` returns a confidence-gap supersession recommendation for conflicting cells with a large confidence gap.
- Missing left or right IDs return `CellNotFound`.
- Non-conflicting cells return `Ok(None)`.

## Roadmap Impact

This adds a Native API milestone for read-only revision conflict analysis. It moves `revise` closer to the embeddable API without introducing mutation semantics before durable revision-link storage is designed.
