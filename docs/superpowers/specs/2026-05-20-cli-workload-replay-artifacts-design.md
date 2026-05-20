# CLI Workload Replay Artifacts Design

## Context

Workload artifact bundles preserve measurement output and a manifest, but storage-engine benchmark archives also need the deterministic input corpus and checkout predicate that produced the result. Without those fixtures, a bundle explains what happened but cannot be replayed from the archived directory alone.

## Decision

Extend `measure-workload --artifact-dir <dir>` bundles with:

- `workload-cells.json`: versioned deterministic StateCell corpus plus workload summary.
- `checkout-request.json`: versioned JSON representation of the measured checkout request.

The report exposes both paths and fingerprints under `workload_artifacts`. The root manifest embeds the same metadata so the archive has a single entry point for report, input corpus, request, baseline comparison, and lookup-plan evidence.

Regression-gated non-zero runs write the replay fixtures before returning the regression error, while continuing to avoid baseline recording for rejected runs.

## Non-goals

- Do not add replay execution commands in this slice.
- Do not change workload generation or checkout semantics.
- Do not add raw file-kernel store copies to the bundle.
