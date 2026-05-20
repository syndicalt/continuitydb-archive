# Commit-Scoped Checkout Design

## Goal

Allow checkout requests to materialize continuity slices for one explicit database commit boundary. `CommitId` already exists at the storage layer; checkout should expose it as a first-class deterministic constraint.

## Scope

This slice adds `CheckoutRequest.commit_id` and pushes it into `CellLookup.commit_id`. It does not add commit records, commit summaries, query language syntax, or checkout output metadata for the commit.

## Architecture

`CheckoutRequest` gains:

```rust
pub commit_id: Option<CommitId>
```

`checkout` passes this value into `CellLookup`. Storage kernels already implement commit-ID filtering, so checkout does not need to re-filter manually. Existing ranking, token-budget packing, audit traces, uncertainty, frontier recommendations, and alternatives remain unchanged.

## Semantics

When `commit_id` is `Some`, checkout considers only cells stored under that commit ID. Other filters still apply normally. When `commit_id` is `None`, checkout behavior is unchanged.

Commit-scoped checkout is useful for audit, replay, post-commit inspection, and future query language forms like "checkout the continuity slice produced by commit X."

## Testing

Tests must prove:

- `checkout` pushes `commit_id` into the kernel lookup request.
- Checkout over a memory kernel returns only cells stamped with the requested commit ID.
- Existing checkout tests keep passing with `commit_id: None`.

## Roadmap Impact

This adds the next checkout milestone: transaction-scoped checkout constraints. It connects storage commit identity to the materialization path used by agents and applications.
