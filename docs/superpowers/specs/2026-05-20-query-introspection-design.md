# Query Introspection Design

## Goal

Add read-only accessors for typed query AST values so embedders, bindings, and CLI tools can inspect decoded queries without relying on private fields or reserializing JSON.

## Context

`continuitydb-query` now supports typed query construction, serde, versioned envelopes, and native/CLI execution. `CheckoutQuery` itself remains mostly write-only from Rust: callers can construct it with builders and compile it, but cannot inspect task metadata, requirements, return shape, or optimization after receiving a decoded query value.

That is a weak fit for bindings and agent-facing APIs. A caller that decodes a query file should be able to validate or display the requested task, token budget, scope, and unsupported semantic requests before executing the query.

## Design Options

1. Add read-only accessor methods.
   This is the recommended approach. It preserves private fields and keeps the AST representation free to evolve while exposing stable inspection points.

2. Make `CheckoutQuery` fields public.
   This is simpler but locks the struct layout into the public API and weakens control over invariants.

3. Add conversion into a separate public view type.
   This may become useful for bindings later, but it is unnecessary until there is a binding crate or FFI boundary.

## Selected Behavior

Add accessor methods to `CheckoutQuery`:

```rust
pub fn task(&self) -> &QueryTask
pub fn requirements(&self) -> &QueryRequirements
pub fn return_shape(&self) -> QueryReturnShape
pub fn optimization(&self) -> QueryOptimization
```

The accessors are read-only. They do not change compile semantics, builders, serde representation, or envelope encoding.

## Testing

Add tests in `continuitydb-query` that:

- build a query with non-default requirements, return shape, and optimization;
- assert the accessors expose the same values;
- decode a query envelope and assert the decoded `CheckoutQuery` can be inspected before compilation.

## Roadmap Impact

This adds a Query Language milestone for AST introspection. It makes typed queries more usable as an embeddable API surface and prepares future bindings without exposing internal fields.
