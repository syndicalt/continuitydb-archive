# Text Query File Execution Design

## Goal

Make saved text `CHECKOUT` query files executable through the native API and existing CLI query-file command.

## Context

The query crate now parses strict text query syntax with `parse_query_text`. The native API already owns saved query-file orchestration for raw `ContinuityQuery` JSON and versioned `continuitydb.query` envelopes. The CLI already delegates `checkout-query` to the native API.

The next layer is to let `ContinuityDb::checkout_query_file` recognize text query files and execute them through the same AST path as JSON queries. This keeps text parsing out of the CLI and preserves the rule that all query input formats become `ContinuityQuery` before execution.

## Design Options

1. Extend native query-file decoding to accept text queries.
   This is selected. It keeps CLI behavior thin, gives embedders the same capability, and reuses the existing parser and execution path.

2. Add a new `checkout-text-query` CLI command.
   This would create a parallel operator surface and force users to know file format at command time. The current `checkout-query` command is already a format-polymorphic saved-query executor.

3. Add only byte-level `checkout_query_text` execution.
   This is useful later, but saved query files are the operational path already exposed by native and CLI APIs. File execution should be closed first.

## Selected Behavior

`ContinuityDb::checkout_query_file` supports three saved query formats:

- versioned `continuitydb.query` JSON envelopes;
- raw `ContinuityQuery` JSON;
- strict text query files accepted by `parse_query_text`.

Detection order:

1. If UTF-8 text begins with `CHECKOUT` case-insensitively after leading whitespace, parse as text query.
2. Otherwise, if JSON has top-level `format`, `version`, and `query`, parse as a versioned query envelope.
3. Otherwise, parse as raw `ContinuityQuery` JSON.

This preserves existing JSON behavior while making intentional text query files explicit.

## Error Handling

Add this native error variant:

```rust
QueryText(QueryTextError)
```

Text parser failures surface as `ContinuityError::QueryText(...)`. Existing file I/O, envelope validation, raw JSON, and query compilation errors keep their current boundaries.

Invalid UTF-8 is not a text query and continues down the JSON decode path, eventually producing `QueryJson` unless it is valid envelope-shaped JSON.

## CLI Behavior

`continuitydb checkout-query <store-path> <query-path>` remains the command. Because it already calls `ContinuityDb::checkout_query_file`, text query support appears automatically after the native helper changes.

## Testing

Add native API tests that:

- write a text query file and assert `checkout_query_file` materializes the expected slice;
- write an invalid text query file beginning with `CHECKOUT` and assert `ContinuityError::QueryText(QueryTextError::InvalidSyntax)`;
- keep existing raw JSON and envelope tests passing.

Add a CLI test that writes a text query file and asserts `continuitydb checkout-query` materializes the expected slice.

## Roadmap Impact

This adds a Native API milestone and updates the CLI query-file milestone to include text query files. It does not add new query syntax beyond the parser slice and does not add a separate text-query command.
