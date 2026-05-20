# Continuity Query AST Design

## Goal

Add the first native typed query boundary for ContinuityDB checkout semantics. This slice should represent the conceptual `CHECKOUT` operation as structured Rust data and compile it into the existing deterministic `CheckoutRequest`.

## Scope

This slice adds a `continuitydb-query` crate with a typed AST and compiler for checkout queries. It does not add a text parser, SQL compatibility, GraphQL compatibility, network server, async runtime, query optimizer, or new storage-kernel lookup capability.

The crate should be the semantic target that future syntax compiles into. The first supported execution target is only `continuitydb-checkout::CheckoutRequest`.

## Architecture

`continuitydb-query` owns query-facing types:

- `ContinuityQuery`: top-level query enum.
- `CheckoutQuery`: structured checkout operation.
- `QueryTask`: task identity and answerability intent.
- `QueryRequirements`: deterministic constraints that map to checkout filters.
- `QueryReturnShape`: requested materialization shape.
- `QueryOptimization`: requested optimization policy.

The initial compiler is deliberately narrow:

```rust
impl CheckoutQuery {
    pub fn compile_checkout(self) -> Result<CheckoutRequest, QueryError>
}
```

`ContinuityQuery::compile_checkout` should delegate when the variant is `Checkout`.

## Semantics

A checkout query must make the task explicit. `QueryTask` carries:

- `name`: stable task name or identifier.
- `answerability_question`: the exact question or intent that selected StateCells should answer.

`QueryRequirements` maps directly to current deterministic checkout constraints:

- `scope` -> `CheckoutRequest.scope`
- `valid_at` -> `CheckoutRequest.valid_at`
- `system_at` -> `CheckoutRequest.system_at`
- `commit_id` -> `CheckoutRequest.commit_id`
- `evidence_source` -> `CheckoutRequest.evidence_source`
- `dependency_target` -> `CheckoutRequest.dependency_target`
- `dependency_kind` -> `CheckoutRequest.dependency_kind`
- `minimum_confidence` -> `CheckoutRequest.minimum_confidence`
- `token_budget` -> `CheckoutRequest.token_budget`

The compiled request receives `answerability_question` from `QueryTask.answerability_question`. This keeps task identity separate from the exact answerability filter while still compiling to the current checkout API.

The first supported return shape is `PackedContextWithMetadata`, meaning the current `CheckoutSlice` shape with selected cells, audit traces, uncertainty, frontier recommendations, and alternatives. The compiler must reject unsupported return shapes with a typed error instead of silently ignoring them.

The first supported optimization is `DeterministicUtility`, meaning the current deterministic checkout ranking by utility and confidence. The compiler must reject unsupported optimization policies with a typed error.

## Defaults

The crate should expose `Default` for `QueryRequirements`:

- no scope, valid-time, system-time, commit, evidence-source, or dependency filters;
- minimum confidence defaults to `Confidence::new(0.0)`;
- token budget defaults to `i64::MAX`.

`CheckoutQuery::new(task)` should use default requirements, `PackedContextWithMetadata`, and `DeterministicUtility`.

## Error Handling

`QueryError` should be a public typed error enum. Initial variants:

- `UnsupportedReturnShape(QueryReturnShape)`
- `UnsupportedOptimization(QueryOptimization)`

The compiler must not panic, unwrap, or coerce unsupported query semantics into weaker behavior.

## Testing

Implementation is test-first. Tests must prove:

- a minimal checkout query compiles task answerability and defaults correctly;
- scope, valid-time, system-time, commit ID, confidence, and token budget compile correctly;
- evidence and dependency requirements compile correctly;
- `ContinuityQuery::compile_checkout` delegates to the checkout variant;
- unsupported return shapes fail with `QueryError::UnsupportedReturnShape`;
- unsupported optimization policies fail with `QueryError::UnsupportedOptimization`.

## Roadmap Impact

This adds the first Query Language milestone without premature parser work. Future SQL-like syntax, API bindings, and agent-facing query builders can compile into this typed AST instead of bypassing ContinuityDB semantics or directly constructing `CheckoutRequest`.
