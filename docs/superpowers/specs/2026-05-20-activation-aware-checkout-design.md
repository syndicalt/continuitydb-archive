# Activation-Aware Checkout Design

## Goal

Make activation state a first-class checkout and query constraint so callers can materialize active, frontier, dormant, or retired StateCells intentionally.

## Context

The storage kernel already supports `CellLookup.activation`, and the durable file kernel maintains activation indexes. Checkout currently cannot request an activation state, so applications must either bypass checkout or post-filter results. That weakens the database-level frontier workflow: the Steward can mark cells as `Frontier`, but checkout cannot directly ask for frontier-only slices.

This slice threads activation through the deterministic checkout boundary, typed query AST, and strict text query parser.

## Selected Behavior

Add an optional activation filter to:

- `CheckoutRequest`
- `QueryRequirements`
- `CheckoutQuery::compile_checkout`
- strict text `CHECKOUT` constraints

Text syntax:

```text
activation = frontier
```

Supported activation identifiers:

- `dormant`
- `active`
- `frontier`
- `retired`

The checkout engine pushes activation into `CellLookup`, preserving the existing storage-index boundary.

## Design Options

1. Add activation to checkout, typed queries, and text queries in one vertical slice.
   This is selected. It keeps all public query paths aligned and uses existing storage indexes.

2. Add only a native API helper for frontier checkout.
   This would create a special-purpose shortcut while leaving the general semantic query model incomplete.

3. Keep activation as storage-only metadata.
   This forces applications to bypass deterministic checkout or post-filter, which contradicts the goal of database-owned context materialization.

## Error Handling

Unknown activation identifiers in text queries return `QueryTextError::InvalidValue` because the field is recognized and the value is unsupported.

Malformed activation syntax returns `QueryTextError::InvalidSyntax`.

Typed query JSON remains backward-compatible because the new field is optional and defaults to `None`.

## Testing

Add checkout tests proving activation is pushed into `CellLookup` and filters selected cells.

Add query tests proving typed activation requirements compile into checkout requests and text `activation = frontier` parses into `QueryRequirements`.

Existing native API and CLI query execution paths should work without direct code changes because they compile and execute the typed AST.

## Roadmap Impact

Add a Checkout milestone for activation-aware checkout constraints and a Query Language milestone for activation constraints in typed and text queries. Update README current scope to mention activation-aware checkout and text query constraints.
