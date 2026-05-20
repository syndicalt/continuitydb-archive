# CLI Query Envelope Design

## Goal

Update `continuitydb checkout-query` so it can execute versioned query-envelope JSON files produced by `continuitydb-query::encode_query_json`.

## Context

The CLI currently accepts raw serialized `ContinuityQuery` JSON. The query crate now exposes `QueryEnvelope`, `encode_query_json`, and `decode_query_json`, which gives query files a stable `format` and `version` boundary. The CLI should use that boundary so operator workflows exercise the same format that bindings and future agents should prefer.

## Design Options

1. Prefer envelopes while preserving raw AST compatibility.
   This is the recommended approach. The command first attempts `decode_query_json`; if the bytes are not a valid envelope, it falls back to raw `ContinuityQuery` decoding. Existing raw query files keep working, while new envelope files become operational immediately.

2. Require envelopes only.
   This is cleaner long term, but it breaks the raw query-file command added one slice earlier. It is better to deprecate raw AST input later once envelope files have been supported for at least one milestone.

3. Add a second command such as `checkout-query-envelope`.
   This avoids ambiguity but creates unnecessary CLI surface area for one semantic operation. The file format should vary, not the operation name.

## Selected Behavior

`continuitydb checkout-query <store-path> <query-path>` should:

1. read the query file bytes;
2. call `continuitydb_query::decode_query_json(&bytes)`;
3. if that succeeds, execute the returned `ContinuityQuery`;
4. if envelope decoding fails, try `serde_json::from_slice::<ContinuityQuery>(&bytes)` as a raw compatibility path;
5. execute through `ContinuityDb::checkout_continuity_query`;
6. print the existing `CheckoutSlice` JSON.

Invalid envelope metadata should cause failure when the file has envelope shape. The fallback is only for raw AST files. That prevents a misspelled `format` or unsupported `version` from being accidentally treated as some other query.

## Error Handling

Add a CLI-local decode helper that distinguishes envelope-shaped JSON from raw AST JSON by checking for object fields named `format`, `version`, and `query`. If those fields are present, use envelope decoding and return envelope errors directly. Otherwise decode raw AST JSON.

This keeps behavior deterministic and avoids using “try envelope then try raw” in a way that masks invalid envelope files.

## Testing

Add CLI tests that:

- write an envelope query file using `encode_query_json` and assert `checkout-query` returns the selected cell;
- write an envelope-shaped file with an unsupported version and assert `checkout-query` fails with `query envelope is invalid`;
- keep the existing raw AST query test passing to prove compatibility.

## Roadmap Impact

This adds a CLI milestone for versioned query-envelope execution. It closes the loop between query envelope helpers and the operator-facing query command without introducing text syntax.
