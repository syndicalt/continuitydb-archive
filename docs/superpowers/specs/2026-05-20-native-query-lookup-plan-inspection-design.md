# Native Query Lookup Plan Inspection Design

## Problem

`ContinuityDb<FileKernel>::file_lookup_plan` exposes indexed candidate planning, but embedders must manually translate checkout query constraints into `CellLookup`. That duplicates semantic mapping outside the native API and risks planner diagnostics diverging from checkout query execution.

## Design

- Add native file-backed lookup-plan helpers for `CheckoutQuery`, top-level `ContinuityQuery`, and strict text `CHECKOUT` input.
- Compile queries through the existing `continuitydb-query` compiler.
- Convert the resulting `CheckoutRequest` into `CellLookup` in one native API helper.
- Preserve the existing raw `CellLookup` lookup-plan method for low-level embedders.
- Keep zero default `minimum_confidence` out of explicit lookup constraints so default query behavior does not inflate indexed constraint counts.
- Refactor the CLI query lookup-plan path to use the native text helper.

The helpers inspect candidate planning only. They do not execute checkout, materialize cells, or mutate the store.

## Test

- Build a file-backed database with one matching project-scoped answerability cell, one broad cell, and one wrong-scope cell.
- Call `file_lookup_plan_for_query_text` with a strict `CHECKOUT` query constrained by answerability and project scope.
- Assert the plan reports two indexed constraints, one candidate, and `full_scan = false`.
- Keep the CLI query lookup-plan regression passing through the native helper.
