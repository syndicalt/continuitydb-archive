# Native Query Execution Design

## Goal

Let embedders execute typed Continuity queries through `ContinuityDb<K>` instead of manually compiling query AST values into checkout requests. This makes the new query boundary part of the native database API.

## Scope

This slice adds a native API bridge from `continuitydb-query` to `continuitydb-api`. It does not add text parsing, CLI query commands, query planning, async execution, or new checkout semantics.

## Architecture

`continuitydb-query` remains the owner of query AST and compilation. `continuitydb-api` depends on it and exposes two convenience operations for any `StorageKernel`:

```rust
pub fn checkout_query(&self, query: CheckoutQuery) -> Result<CheckoutSlice, ContinuityError>
pub fn checkout_continuity_query(
    &self,
    query: ContinuityQuery,
) -> Result<CheckoutSlice, ContinuityError>
```

Both methods compile the query into `CheckoutRequest` and then delegate to the existing `checkout` method. There is no duplicate checkout logic in the API crate.

## Semantics

`checkout_query` accepts a structured checkout query and returns the same `CheckoutSlice` that direct checkout would return for the compiled request.

`checkout_continuity_query` accepts the top-level query enum. The only current query variant is checkout, so it delegates through `ContinuityQuery::compile_checkout`.

Unsupported query semantics, such as `QueryReturnShape::CellsOnly` or `QueryOptimization::TokenCostOnly`, must return a typed API error before storage lookup. The native API must not silently downgrade unsupported query semantics.

## Error Handling

`ContinuityError` gains:

```rust
Query(QueryError)
```

The new variant wraps `continuitydb_query::QueryError`. Query compilation failures should propagate as `ContinuityError::Query`, while storage and checkout failures continue to use the existing variants.

## Testing

Implementation is test-first. Tests must prove:

- `checkout_query` returns a slice selected from ingested cells;
- `checkout_query` applies query requirements such as scope, confidence, and token budget through the compiled `CheckoutRequest`;
- `checkout_continuity_query` delegates from the top-level query enum;
- unsupported query return shapes fail with `ContinuityError::Query(QueryError::UnsupportedReturnShape(...))`;
- unsupported query optimization policies fail with `ContinuityError::Query(QueryError::UnsupportedOptimization(...))`.

## Roadmap Impact

This adds the second Query Language milestone. The typed query AST becomes executable through the native database API, creating a stable semantic path for future text syntax, CLI query commands, and agent-facing query builders.
