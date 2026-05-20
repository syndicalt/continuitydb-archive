# Dependency Kind Lookup Design

## Problem

`CellLookup.dependency_kind` is a public storage lookup constraint and strict text
queries can parse `dependency_kind = ...`, but memory and file storage currently
apply dependency kind only when `dependency_target` is also present. A kind-only
lookup silently behaves like an unconstrained dependency lookup.

## Decision

Make dependency kind a first-class storage lookup constraint:

- `dependency_kind` alone matches cells with any dependency of that kind.
- `dependency_target` alone keeps matching cells with any dependency to that target.
- `dependency_target` plus `dependency_kind` keeps matching cells with a dependency
  satisfying both fields on the same dependency edge.
- The file kernel maintains a dependency-kind secondary index and exposes it in
  `FileKernelLookupPlan` as `dependency_kind`.

## Verification

Add tests that fail before implementation:

- Memory kernel kind-only lookup returns only cells with a matching dependency kind.
- File kernel kind-only lookup survives reopen and returns only matching cells.
- File kernel lookup plan reports `dependency_kind` as an indexed constraint with
  the expected candidate count.
