# Native Query File Execution Design

## Goal

Let embedders execute saved typed query files through the native API without duplicating CLI-only file reading, envelope detection, or raw-query compatibility logic.

## Context

ContinuityDB now supports typed query AST execution, JSON serialization, versioned `continuitydb.query` envelopes, CLI query files, CLI query envelopes, and native execution of envelope bytes. The remaining gap is file-level native ergonomics: the CLI can execute either raw `ContinuityQuery` JSON or a versioned query envelope from disk, but embedders still need to copy the binary's private `read -> detect envelope shape -> decode -> execute` flow.

This is mislayered. The CLI should stay thin over the embeddable API. File-backed query execution is an embeddable datastore operation because query files are a portable persistence boundary, not just a command-line concern.

## Design Options

1. Add `ContinuityDb<K>::checkout_query_file`.
   This is the selected approach. It works for memory-backed and file-backed databases, reads a saved query file from disk, preserves the CLI compatibility behavior, and returns the same checkout slice as the byte-level native query APIs.

2. Add `ContinuityDb<FileKernel>::checkout_query_file`.
   This keeps file helpers near other file-backed APIs, but it unnecessarily ties query-file execution to the file storage kernel. A query file can be executed against any open database handle.

3. Add only a `decode_query_file` helper in `continuitydb-query`.
   This would reduce duplication, but embedders would still perform a two-step decode-then-execute flow. The native API should own common operation orchestration.

## Selected Behavior

Add this method on `ContinuityDb<K: StorageKernel>`:

```rust
pub fn checkout_query_file<P: AsRef<Path>>(
    &self,
    query_path: P,
) -> Result<CheckoutSlice, ContinuityError>
```

The method reads the file, decodes either a versioned `continuitydb.query` envelope or raw `ContinuityQuery` JSON, then executes the decoded query through `checkout_continuity_query`.

Compatibility rule:

- If JSON has top-level `format`, `version`, and `query` fields, treat it as a query envelope and validate with `decode_query_json`.
- Otherwise treat it as raw `ContinuityQuery` JSON.

This intentionally matches current CLI behavior so users can move a query file between CLI, tests, bindings, and embedded applications.

## Error Handling

Add two native error variants:

```rust
QueryJson
QueryFileIo
```

`QueryFileIo` covers file-read failures. `QueryJson` covers malformed raw query JSON and malformed JSON before envelope-shape detection. Existing `QueryEnvelope(QueryEnvelopeError)` remains responsible for syntactically valid envelope-shaped JSON that fails envelope validation. Existing `Query(QueryError)` remains responsible for query compilation or unsupported semantic choices.

The native helper should avoid leaking `serde_json::Error` or `std::io::Error` in the public error surface.

## CLI Refactor

Refactor `continuitydb checkout-query <store-path> <query-path>` to call:

```rust
let db = ContinuityDb::open_file(store_path)?;
let slice = db.checkout_query_file(query_path)?;
```

The CLI should stop owning query-file decoding. Its tests should continue to prove raw query files and versioned envelopes both execute.

## Testing

Add native API tests that:

- write a versioned query envelope file and assert `checkout_query_file` materializes the expected slice;
- write a raw `ContinuityQuery` JSON file and assert compatibility still works;
- pass a missing path and assert `ContinuityError::QueryFileIo`;
- write malformed raw JSON and assert `ContinuityError::QueryJson`;
- write an envelope-shaped file with an unsupported version and assert `ContinuityError::QueryEnvelope(QueryEnvelopeError::InvalidEnvelope)`.

Keep existing CLI tests for query-file and query-envelope execution. After the CLI refactor, those tests prove the command still routes through the native helper correctly.

## Roadmap Impact

This adds a Native API milestone for saved query-file execution and updates the CLI milestone to note that `checkout-query` is routed through the embeddable API boundary. It moves ContinuityDB closer to the intended layering: core datastore behavior lives in libraries, while the CLI is operational sugar.
