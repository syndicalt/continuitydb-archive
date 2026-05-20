# Core Revision Link Record Design

## Purpose

ContinuityDB currently has in-memory revision graphs in `continuitydb-revision`, and accepted Steward `LinkRevision` proposals are materialized as operational StateCells. The roadmap calls out this as a temporary stand-in until storage grows native revision-link records.

This slice establishes the stable semantic record in `continuitydb-core` before changing any storage kernel. It gives future kernels, APIs, sync/export paths, and Steward application code one shared data shape for append-only revision links.

## Architecture

Move the revision link kind into the core semantic layer and add `RevisionLinkRecord`:

- `source`: source StateCell version.
- `target`: target StateCell version.
- `kind`: revision relationship.
- `recorded_at`: system time when the link was committed or observed.

`continuitydb-revision` should re-export `RevisionLinkKind` from core so existing consumers keep compiling while the semantic owner moves downward in the crate graph.

This slice does not persist revision links in kernels yet. Kernel persistence should follow once the core type is committed and verified.

## Tests

Tests must prove:

- Core can create and serialize a `RevisionLinkRecord`.
- The record preserves source, target, kind, and recorded time.
- `continuitydb_revision::RevisionLinkKind` remains available to existing callers.

## Roadmap Placement

Add a Dependency and Causality milestone for the core native revision-link record.
