# API Utility Feedback Design

## Goal

Expose utility feedback as a native embeddable ContinuityDB API operation. Applications should be able to record outcome feedback for a stored `StateCell` without manually looking up cells, calling revision helpers, and appending successor versions.

## Scope

This slice adds a typed API operation to `continuitydb-api`. It does not add a query language, persistent revision graph storage, automatic belief fusion, or model-driven feedback.

## Architecture

`continuitydb-api` will depend on `continuitydb-revision` and use the existing `revise_utility_feedback` helper. `ContinuityDb<K>` gains:

- `record_utility_feedback`
- `record_utility_feedback_at`

The deterministic `_at` variant is the core operation. It resolves the current cell by immutable ID, creates a successor `StateCell` carrying the provided `UtilityFeedback`, appends that successor through the backing `StorageKernel`, and returns the successor ID.

## Semantics

Feedback is append-only. The prior cell is not mutated. The successor preserves the prior cell's anchors, valid time, scope, evidence, payload, costs, answerability, activation, and dependencies while changing the ID and utility feedback. The successor's system time is assigned by the kernel commit path.

Missing prior cells return `ContinuityError::CellNotFound`. Duplicate append failures and storage failures continue to surface as `ContinuityError::Kernel`.

## Testing

Tests must prove:

- Recording feedback creates a new cell ID.
- The new cell is queryable by ID from the kernel.
- The new cell carries the requested utility feedback.
- The old cell remains stored with its original feedback.
- The deterministic commit time is applied to the new successor.
- Missing prior IDs return `CellNotFound`.

## Roadmap Impact

This adds a second Native API milestone: outcome feedback can now drive append-only utility revision from the embeddable API. This closes the first native loop from checkout to action outcome to revised future checkout utility.
