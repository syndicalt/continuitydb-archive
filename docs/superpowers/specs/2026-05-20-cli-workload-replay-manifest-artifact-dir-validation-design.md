# CLI Workload Replay Manifest Artifact Directory Validation Design

## Problem

`replay-workload --require-manifest` validates required bundle report and fixture paths, but it still accepts a workload bundle manifest whose top-level `artifact_dir` points somewhere other than the replayed artifact directory.

## Desired Behavior

When `--require-manifest` is set, replay validates that the manifest `artifact_dir` is exactly the requested `--artifact-dir`.

## Scope

- Reject self-inconsistent workload bundle manifests before replaying fixtures.
- Preserve existing report path, fixture path, and fixture fingerprint validation.
- Keep validation deterministic and local to the required manifest validation path.

## Out of Scope

- Canonicalizing or rewriting artifact paths.
- Validating kernel or store path fields, because replay may intentionally compare a bundle across kernels.
- Changing workload bundle format version.
