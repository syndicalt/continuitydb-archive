# Query Envelope Design

## Goal

Add a versioned JSON envelope for serialized `ContinuityQuery` values so saved query files, bindings, and agent APIs have a stable format boundary before text query syntax exists.

## Context

The typed query AST is serializable and the CLI can execute raw `ContinuityQuery` JSON files. Raw AST JSON is useful for the first operational path, but it does not identify the payload as a ContinuityDB query file or provide a version check. Commit export already uses a versioned envelope in `continuitydb-api`; query files need the same kind of explicit wire-format contract.

## Design Options

1. Add a small envelope type in `continuitydb-query`.
   This is the recommended approach. The query crate owns query wire semantics, so callers can encode, decode, and validate query files without depending on the CLI or API crates.

2. Add envelope handling only in the CLI.
   This would solve one command but would strand bindings and embedders. It also puts query format validation in an operational shell instead of the query semantic crate.

3. Delay envelopes until text syntax exists.
   This keeps the current raw JSON surface simple, but it lets unversioned query files spread before the format is named.

## Selected Shape

Add these public constants and type to `continuitydb-query`:

```rust
pub const QUERY_ENVELOPE_FORMAT: &str = "continuitydb.query";
pub const QUERY_ENVELOPE_FORMAT_VERSION: u32 = 1;

pub struct QueryEnvelope {
    pub format: String,
    pub version: u32,
    pub query: ContinuityQuery,
}
```

`QueryEnvelope::new(query)` wraps a query in the current format and version. `validate()` accepts only `format == "continuitydb.query"` and `version == 1`.

Add crate-level helpers:

```rust
pub fn encode_query_json(query: ContinuityQuery) -> Result<Vec<u8>, QueryEnvelopeError>
pub fn decode_query_json(bytes: &[u8]) -> Result<ContinuityQuery, QueryEnvelopeError>
```

The helpers encode and decode the versioned envelope, not raw AST JSON. The raw AST serde support remains available for advanced embedders and tests.

## Error Handling

Add `QueryEnvelopeError` with variants for invalid JSON and invalid envelope metadata. The error should not panic, should not expose `serde_json::Error` in public equality semantics, and should support deterministic tests.

## Compatibility

This slice does not change the CLI yet. Existing `checkout-query` continues to accept raw `ContinuityQuery` JSON. A later CLI slice can accept envelope files, either by requiring envelopes or by adding a clear compatibility path.

## Testing

Add tests in `continuitydb-query` that prove:

- encoding a query creates a JSON envelope with `format`, `version`, and `query`;
- decoding a current envelope returns the original `ContinuityQuery`;
- unsupported format or version is rejected;
- malformed JSON is rejected as invalid JSON.

## Roadmap Impact

This adds a Query Language milestone for a versioned query file envelope. It prepares bindings and CLI query files for format evolution without introducing text syntax or parser behavior.
