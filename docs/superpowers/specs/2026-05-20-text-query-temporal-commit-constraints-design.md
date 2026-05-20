# Text Query Temporal and Commit Constraints Design

## Goal

Extend strict text `CHECKOUT` queries with temporal and commit-boundary constraints already supported by the typed query AST.

## Context

`QueryRequirements` supports `valid_at`, `system_at`, and `commit_id`, and checkout compilation already maps those fields into deterministic `CheckoutRequest` constraints. The text parser currently supports scope, minimum confidence, token budget, and evidence source only. That makes saved text queries less expressive than serialized typed queries for audit and replay workflows.

This slice keeps text syntax strict and explicit while closing the most important bitemporal gap.

## Selected Syntax

Add these `WHERE` constraints:

```text
valid_at = "2026-05-20T12:00:00Z"
system_at = "2026-05-20T12:30:00Z"
commit_id = "550e8400-e29b-41d4-a716-446655440000"
```

Timestamps must be RFC3339 strings and are normalized to UTC. Commit IDs must parse with the existing `CommitId` `FromStr` implementation. The syntax uses quoted strings instead of unquoted timestamp or UUID tokens so the lexer can stay small and deterministic.

## Design Options

1. Add quoted string constraints for temporal and commit fields.
   This is selected. It reuses the existing lexer, keeps syntax readable, and avoids introducing specialized timestamp or UUID token kinds.

2. Add function syntax such as `valid_at = time("...")`.
   This is more explicit but adds grammar surface without improving the typed AST boundary.

3. Defer temporal syntax until a larger query language redesign.
   This would leave text queries materially weaker than typed JSON queries for audit and replay tasks.

## Error Handling

Malformed constraint syntax returns `QueryTextError::InvalidSyntax`.

Syntactically valid but unparsable values return `QueryTextError::InvalidValue`, including invalid RFC3339 timestamps and invalid commit UUID text.

## Testing

Add parser tests that:

- parse `valid_at`, `system_at`, and `commit_id` into `QueryRequirements`;
- reject invalid timestamp and commit values as `InvalidValue`;
- keep existing text query tests passing.

No native API or CLI changes are needed because both already route text input through `parse_query_text`.

## Roadmap Impact

Add a Query Language milestone for text temporal and commit constraints. Update README current scope to mention bitemporal and commit-scoped text query constraints.
