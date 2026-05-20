# CLI Query Files Design

## Goal

Make the typed Continuity Query AST usable from the command line by adding a file-backed CLI command that reads serialized query JSON and executes it through the native typed query API.

## Context

`continuitydb-query` now supports serde for the typed query AST, and `continuitydb-api` can execute `ContinuityQuery` values through `ContinuityDb::checkout_continuity_query`. The CLI still has no path from a saved typed query file to real checkout output. That blocks early operator workflows, bindings tests, and future agent-facing query builders from exercising the typed query boundary without writing Rust.

## Design Options

1. Add `checkout-query <store-path> <query-path>` now.
   This is the recommended approach. It is small, deterministic, and uses the existing JSON representation directly. It avoids text parser design and proves the AST serialization boundary with an end-to-end workflow.

2. Add a versioned query envelope first.
   This is valuable soon, but it creates a schema-design slice before there is a user-visible query execution path. It can follow once the plain AST command exposes the operational shape.

3. Add SQL-like or CHECKOUT syntax parsing.
   This is premature. The typed AST is the semantic target; parser work should wait until the command/file execution path has stabilized.

## Selected Behavior

Add a CLI command:

```text
continuitydb checkout-query <store-path> <query-path>
```

The command opens `<store-path>` as the current file-backed database, reads `<query-path>` as JSON, decodes it into `continuitydb_query::ContinuityQuery`, executes it with `ContinuityDb::checkout_continuity_query`, and prints the resulting `CheckoutSlice` as pretty JSON.

The command accepts the serde representation already established for the AST. A minimal query file looks like:

```json
{
  "checkout": {
    "task": {
      "name": "stored-facts",
      "answerability_question": "what is stored?"
    },
    "requirements": {
      "scope": { "Project": "continuitydb" },
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

## Error Handling

Invalid JSON, unsupported query semantics, missing files, and storage or checkout failures should cause a non-zero CLI exit through the existing `run()` error path. The first implementation does not need custom user-facing error variants; it should preserve the underlying typed API errors rather than silently downgrading unsupported query requests.

## Testing

Add CLI tests that:

- write a committed file-backed store with a StateCell answerable by `"what is stored?"`;
- write a typed query JSON file;
- run `continuitydb checkout-query <store> <query>`;
- assert the output is a `CheckoutSlice` JSON with one selected cell and audit metadata;
- write an unsupported query shape and assert the command exits non-zero.

## Roadmap Impact

This adds a Query Language milestone for CLI execution of typed query files. It makes serialized typed queries operational before text syntax, envelopes, or external bindings are added.
