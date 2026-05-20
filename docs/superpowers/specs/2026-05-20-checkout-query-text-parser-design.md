# Checkout Query Text Parser Design

## Goal

Add the first human-authored Continuity Query text syntax by parsing a strict, minimal `CHECKOUT` form into the existing typed query AST.

## Context

ContinuityDB now has a typed query AST, serde support, versioned query envelopes, CLI query-file execution, native envelope execution, and native saved query-file execution. The remaining Query Language gap is text syntax. The parser should not bypass the AST or checkout compiler; text must compile into `ContinuityQuery` so all execution paths share one semantic target.

## Design Options

1. Add a narrow parser in `continuitydb-query` for one `CHECKOUT` form.
   This is selected. It establishes the text-query boundary without changing storage, checkout, native API, or CLI execution behavior. The parser can grow by extending the AST compiler, not by teaching execution layers new semantics.

2. Add SQL-like syntax now.
   This is too broad. SQL wording would imply relational semantics ContinuityDB does not have and would force premature choices around projection, joins, and optimizer language.

3. Add CLI-only text parsing.
   This would repeat the layering mistake already fixed for query files. Text parsing belongs in `continuitydb-query` so native embedders, CLI, bindings, and agents use the same parser.

## Selected Syntax

The first grammar is intentionally strict:

```text
CHECKOUT "task-name" ANSWER "answerability question"
```

Optional constraints may be appended in a `WHERE` clause:

```text
CHECKOUT "release" ANSWER "what should ship?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
  AND token_budget <= 1200
```

Supported constraints in this slice:

- `scope = project("name")`
- `scope = team("name")`
- `scope = org("name")`
- `scope = personal("name")`
- `scope = task("name")`
- `scope = global`
- `min_confidence >= <0.0..1.0>`
- `token_budget <= <non-negative integer>`
- `evidence_source = "source-id"`

Unsupported constraints are rejected instead of ignored.

## Parser Boundary

Add this public API to `continuitydb-query`:

```rust
pub fn parse_query_text(input: &str) -> Result<ContinuityQuery, QueryTextError>
```

Add this error type:

```rust
pub enum QueryTextError {
    InvalidSyntax,
    InvalidValue,
}
```

`InvalidSyntax` covers malformed grammar and unknown fields/operators. `InvalidValue` covers syntactically valid values that violate existing domain constraints, such as confidence outside `0.0..=1.0` or negative token budget.

## Implementation Approach

Use a small explicit lexer plus parser in `continuitydb-query`, not substring splitting in the CLI. The lexer recognizes identifiers, quoted strings, numbers, parentheses, `=`, `>=`, `<=`, and keywords case-insensitively. The parser consumes that token stream according to the grammar and builds `CheckoutQuery` plus `QueryRequirements`.

This keeps parsing deterministic, testable, and isolated. It avoids a new dependency for the first small grammar while still avoiding loose ad hoc string manipulation.

## Integration

This slice only adds parser support in `continuitydb-query`. Query execution can already happen by passing the parsed `ContinuityQuery` into the native API. CLI text-file autodetection is a later slice so the first parser commit remains focused and easy to audit.

## Testing

Add query crate tests that prove:

- minimal `CHECKOUT "task" ANSWER "question"` parses into a checkout query with default requirements;
- supported `WHERE` constraints populate scope, minimum confidence, token budget, and evidence source;
- keywords are case-insensitive;
- malformed syntax returns `QueryTextError::InvalidSyntax`;
- invalid confidence or token budget returns `QueryTextError::InvalidValue`.

Existing compile tests continue to prove the parsed AST feeds the checkout compiler.

## Roadmap Impact

This adds a Query Language milestone for first text syntax parsing. It does not add SQL compatibility, CLI text autodetection, natural-language query interpretation, or new checkout semantics.
