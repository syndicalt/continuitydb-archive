# CLI Query Lookup Plan Inspection Design

## Problem

`inspect-kernel --lookup-plan` exposes planner metadata, but only for the default unconstrained lookup. Operators need to inspect whether real checkout predicates use indexes before running materialization or adding workload regression gates.

## Design

- Add `inspect-kernel --lookup-query <text>`.
- Parse the value with the existing strict text `CHECKOUT` parser.
- Compile the query into `CheckoutRequest`.
- Convert checkout constraints into `CellLookup` for file-kernel planning.
- Keep `minimum_confidence` out of the lookup plan when it is the zero default so default query behavior does not appear as an extra explicit indexed constraint.
- Return the same `lookup_plan` JSON shape as `--lookup-plan`.

This does not execute checkout and does not materialize cells. It only inspects the candidate plan.

## Test

- Build a file store with two project-scoped cells answering `what is stored?`.
- Run `continuitydb inspect-kernel <store> --lookup-query 'CHECKOUT "inspect" ANSWER "what is stored?" WHERE scope = project("continuitydb")'`.
- Assert the plan reports two indexed constraints, two candidates, and `full_scan = false`.
