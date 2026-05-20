# Native Query Text Execution Design

## Goal

Let embedders execute strict text `CHECKOUT` queries directly through the native API without creating saved query files.

## Context

ContinuityDB now has one semantic query target: text syntax, JSON envelopes, raw query JSON, and typed in-process calls all become `ContinuityQuery` before checkout materialization. The native file helper already recognizes strict text query files, but library callers with query text in memory still need to write a temporary file or call the parser crate directly.

The next layer is a small ergonomic API on `ContinuityDb<K>`:

```rust
pub fn checkout_query_text(&self, input: &str) -> Result<CheckoutSlice, ContinuityError>
```

The method should parse text with `continuitydb_query::parse_query_text`, then delegate to `checkout_continuity_query`. It must preserve existing error boundaries: parser failures surface as `ContinuityError::QueryText`, and query compilation failures continue to surface as `ContinuityError::Query`.

## Design Options

1. Add direct native text execution on `ContinuityDb<K>`.
   This is selected. It gives embedders the same text-query capability as saved files while keeping all semantics routed through the typed AST.

2. Require embedders to call `parse_query_text` themselves.
   This leaks orchestration into applications and makes native API ergonomics worse than CLI/file execution.

3. Add byte-level format detection that accepts JSON or text in one method.
   This may be useful later, but direct text execution should be explicit so parser errors remain unambiguous and callers do not need format heuristics for in-memory strings.

## Selected Behavior

`ContinuityDb::checkout_query_text` accepts strict text query syntax supported by `parse_query_text`. It returns the same `CheckoutSlice` as the equivalent typed query.

Example:

```text
CHECKOUT "stored-facts" ANSWER "what should the agent know?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
  AND token_budget <= 1200
```

Execution flow:

1. Parse input text into `ContinuityQuery`.
2. Compile the query through `checkout_continuity_query`.
3. Materialize the slice through the existing checkout engine.

No CLI changes are required for this slice because saved text query files are already supported by `checkout-query`.

## Error Handling

Invalid text syntax returns:

```rust
ContinuityError::QueryText(QueryTextError::InvalidSyntax)
```

Syntactically valid but invalid values return:

```rust
ContinuityError::QueryText(QueryTextError::InvalidValue)
```

Unsupported typed query shapes remain `ContinuityError::Query(...)` after parse and compile. The method does not perform JSON or saved-file detection.

## Testing

Add native API tests that:

- ingest a matching StateCell and assert `checkout_query_text` materializes it;
- pass invalid text and assert `ContinuityError::QueryText(QueryTextError::InvalidSyntax)`.

Run the focused red/green command:

```bash
cargo test -p continuitydb-api checkout_query_text --all-features
```

Then run the full verification gate before final reporting.

## Roadmap Impact

Add a Native API milestone for direct strict text query execution. Update README current scope to mention native in-memory text query execution. This does not expand query grammar and does not add a new CLI command.
