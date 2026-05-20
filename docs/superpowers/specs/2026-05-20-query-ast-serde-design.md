# Query AST Serde Design

## Goal

Make the typed Continuity Query AST serializable and deserializable so future CLI commands, config files, bindings, and agent-facing APIs can exchange query values without introducing a text query parser yet.

## Scope

This slice adds serde support to `continuitydb-query`. It does not add a CLI command, parser, schema generator, versioned envelope, network API, or new query semantics.

## Architecture

`continuitydb-query` adds a `serde` dependency and derives `Serialize` and `Deserialize` for its public query AST types:

- `ContinuityQuery`
- `CheckoutQuery`
- `QueryTask`
- `QueryRequirements`
- `QueryReturnShape`
- `QueryOptimization`

The crate should use stable snake-case enum tags:

```rust
#[serde(rename_all = "snake_case")]
```

This keeps JSON-oriented representations aligned with Rust field names while avoiding Rust enum variant casing in external data.

## Semantics

Serialization must preserve every field needed to compile a query into `CheckoutRequest`.

The top-level `ContinuityQuery` representation should use serde's external enum tagging with snake-case variant names, for example:

```json
{
  "checkout": {
    "task": {
      "name": "release-readiness",
      "answerability_question": "what is the release status?"
    },
    "requirements": {
      "scope": {"Project": "continuitydb"},
      "valid_at": null,
      "system_at": null,
      "commit_id": null,
      "evidence_source": null,
      "dependency_target": null,
      "dependency_kind": null,
      "minimum_confidence": 0.7,
      "token_budget": 1200
    },
    "return_shape": "packed_context_with_metadata",
    "optimization": "deterministic_utility"
  }
}
```

The inner `Scope`, `CommitId`, `StateCellId`, `CellDependencyKind`, and `Confidence` representations remain owned by `continuitydb-core`.

## Error Handling

Serde decode errors should remain serde errors. The query crate does not wrap JSON errors in this slice.

## Testing

Implementation is test-first. Tests must prove:

- a `CheckoutQuery` round-trips through JSON and still compiles to the same `CheckoutRequest`;
- a top-level `ContinuityQuery` serializes with the snake-case `"checkout"` key and deserializes back;
- unsupported-but-representable enum variants such as `cells_only` and `token_cost_only` survive round-trip and still fail compilation with typed `QueryError` values.

## Roadmap Impact

This adds a Query Language milestone for portable typed query representation. It prepares future text syntax, CLI query files, bindings, and agent APIs while keeping parser design out of the current slice.
