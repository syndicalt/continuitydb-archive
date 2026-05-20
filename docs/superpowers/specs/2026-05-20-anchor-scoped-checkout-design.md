# Anchor-Scoped Checkout Design

## Goal

Make semantic anchors first-class checkout and query constraints so callers can materialize StateCells by stable meaning through the deterministic checkout path.

## Context

`CellLookup` already supports `semantic_anchor`, and the file kernel has an ID/semantic-anchor index. `checkout` currently hardcodes `semantic_anchor: None`, so applications cannot ask checkout for cells anchored to a specific operational concept. That leaves one of the StateCell identity primitives available only at the storage layer.

This slice threads anchor constraints through checkout, typed queries, and strict text `CHECKOUT` syntax.

## Selected Behavior

Add optional semantic-anchor constraints to:

- `CheckoutRequest`
- `QueryRequirements`
- `CheckoutQuery::compile_checkout`
- strict text `CHECKOUT` constraints

Text syntax:

```text
semantic_anchor = "project:continuitydb:release-status"
```

Checkout converts the typed `SemanticAnchor` into the storage lookup string and pushes it into `CellLookup.semantic_anchor`.

## Error Handling

Text syntax requires a quoted string. Missing or non-string values return `QueryTextError::InvalidSyntax`.

`SemanticAnchor::new` currently accepts any string, so there is no new invalid-value case for anchors in this slice.

## Testing

Add checkout tests proving semantic anchors are pushed into `CellLookup` and filter selected cells.

Add query tests proving typed semantic-anchor requirements compile into checkout requests and strict text `semantic_anchor = "..."` parses into `QueryRequirements`.

Existing native API and CLI query execution should inherit the behavior through normal query compilation.

## Roadmap Impact

Add Checkout and Query Language milestones for semantic-anchor scoped materialization. Update README current scope to mention anchor-scoped checkout and text query constraints.
