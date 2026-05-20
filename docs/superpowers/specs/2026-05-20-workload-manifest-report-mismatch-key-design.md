# Workload Manifest Report Mismatch Key Design

## Context

`replay-workload --require-manifest` validates that manifest-owned report fields still match the archived `workload-report.json`. The validation includes `lookup_plan`, so newly added planner diagnostics are protected. The current failure message is generic, which forces operators to diff the manifest and report manually.

## Design

Report the mismatched manifest-owned report key in the validation error:

- Keep the existing validation keys unchanged.
- When a key differs, return `workload artifact manifest report content mismatch: <key>`.
- Preserve the existing substring `workload artifact manifest report content mismatch` for compatibility with existing tests and scripts.
- Add a CLI test that tampers `workload-report.json.lookup_plan.candidate_selectivity_basis_points` and asserts the failure mentions `lookup_plan`.

## Scope

This is a diagnostics-only change. It does not alter manifest format, replay behavior, lookup-plan generation, or comparison semantics.

