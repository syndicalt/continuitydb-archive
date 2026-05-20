# Text Query Dependency Constraints Design

## Goal

Extend strict text `CHECKOUT` queries with dependency-aware constraints already supported by the typed query AST and checkout engine.

## Context

`QueryRequirements` already has `dependency_target` and `dependency_kind`, and checkout pushdown already uses those fields. Text queries cannot express them yet, so saved text queries remain weaker than serialized typed queries for causality and dependency inspection.

One supporting gap exists in `continuitydb-core`: `CommitId` has stable text display and parsing, but `StateCellId` does not. Dependency targets are StateCell IDs, so this slice first gives `StateCellId` the same text round-trip capability.

## Selected Syntax

Add these `WHERE` constraints:

```text
dependency_target = "550e8400-e29b-41d4-a716-446655440000"
dependency_kind = depends_on
```

Supported dependency kind identifiers are:

- `depends_on`
- `caused_by`
- `supports`
- `derived_from`

The target is a quoted UUID string parsed as `StateCellId`. The kind is an unquoted identifier to match other enum-like syntax such as `scope = global`.

## Design Options

1. Add text parsing for `StateCellId` plus dependency target and kind constraints.
   This is selected. It closes the existing typed/text query gap and improves core ID ergonomics for future bindings and diagnostics.

2. Parse dependency targets as raw strings in the query crate.
   This would duplicate UUID parsing outside the owning core type and weaken the domain boundary.

3. Defer dependency syntax until a larger query-language redesign.
   This leaves causality and dependency checkout unavailable from the strict text syntax even though the storage and checkout layers already support it.

## Error Handling

Malformed dependency syntax returns `QueryTextError::InvalidSyntax`.

Invalid dependency target UUID text returns `QueryTextError::InvalidValue`.

Unknown dependency kind identifiers return `QueryTextError::InvalidValue` because the field is recognized and the value is unsupported.

## Testing

Add core tests that prove `StateCellId` displays and parses UUID text and rejects invalid UUID text.

Add query parser tests that:

- parse `dependency_target` and `dependency_kind` into `QueryRequirements`;
- reject invalid dependency target UUIDs as `InvalidValue`;
- reject unknown dependency kinds as `InvalidValue`.

No native API or CLI changes are required because both already route text input through `parse_query_text`.

## Roadmap Impact

Add a Query Language milestone for dependency constraints in text checkout queries. Update README current scope to mention dependency-aware strict text query constraints.
