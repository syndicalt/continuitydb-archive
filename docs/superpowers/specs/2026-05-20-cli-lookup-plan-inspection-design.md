# CLI Lookup Plan Inspection Design

## Problem

The file kernel and native API expose lookup-plan metadata, but operators and CI cannot inspect it from the command line. `inspect-kernel` already reports capabilities, status, and health, so planner observability belongs on the same command.

## Design

- Add `inspect-kernel --lookup-plan`.
- Keep lookup-plan output opt-in to avoid changing the default inspection payload more than necessary.
- Emit `lookup_plan` with:
  - `indexed_constraint_count`
  - `candidate_count`
  - `full_scan`
- Use `CellLookup::default()` for this first CLI surface. This proves operator access to planner metadata without adding lookup-query parsing to kernel inspection.

## Test

- Run `continuitydb inspect-kernel <store> --lookup-plan`.
- Assert default lookup planning reports zero indexed constraints, all visible cells as candidates, and `full_scan = true`.
