# Native Query Envelope Execution Design

## Goal

Let embedders execute versioned typed query JSON envelope bytes through `continuitydb-api` without manually depending on query decoding helpers.

## Context

The query crate owns `QueryEnvelope`, `encode_query_json`, and `decode_query_json`. The CLI now uses envelope-aware decoding for `checkout-query`. Native embedders still need to decode query bytes themselves before calling `ContinuityDb::checkout_continuity_query`.

The native API already has commit export JSON helpers because file or wire formats should be easy to use at the embeddable boundary. Query JSON envelopes should get the same treatment.

## Design Options

1. Add `ContinuityDb::checkout_query_json(&self, bytes: &[u8])`.
   This is the recommended approach. It keeps envelope decode and query execution together at the native operation boundary while preserving the query crate as the format owner.

2. Add only decode helper re-exports from `continuitydb-api`.
   This reduces API logic, but still makes embedders perform a two-step decode-then-execute flow for the common operation.

3. Add file-specific query execution helpers now.
   This is useful later, but byte-slice execution is the smaller and more general primitive. File I/O can be layered on top.

## Selected Behavior

Add this method on `ContinuityDb<K: StorageKernel>`:

```rust
pub fn checkout_query_json(&self, bytes: &[u8]) -> Result<CheckoutSlice, ContinuityError>
```

The method decodes bytes using `continuitydb_query::decode_query_json`, maps decode failures into a native API error, then executes the decoded `ContinuityQuery` through `checkout_continuity_query`.

## Error Handling

Extend `ContinuityError` with:

```rust
QueryEnvelope(QueryEnvelopeError)
```

This preserves the difference between query compilation failures and query envelope decode/validation failures. Unsupported return shapes still surface as `ContinuityError::Query(QueryError::UnsupportedReturnShape(...))`; invalid JSON or unsupported envelope metadata surfaces as `ContinuityError::QueryEnvelope(...)`.

## Testing

Add native API tests that:

- encode a valid checkout query envelope and assert `checkout_query_json` returns the expected checkout slice;
- encode an unsupported return shape and assert the method returns `ContinuityError::Query(...)`;
- pass malformed JSON and assert the method returns `ContinuityError::QueryEnvelope(QueryEnvelopeError::InvalidJson)`;
- pass an unsupported envelope version and assert `ContinuityError::QueryEnvelope(QueryEnvelopeError::InvalidEnvelope)`.

## Roadmap Impact

This adds a Native API milestone for executing versioned query envelopes. It makes query files and binding payloads first-class at the embeddable API boundary without introducing text syntax.
