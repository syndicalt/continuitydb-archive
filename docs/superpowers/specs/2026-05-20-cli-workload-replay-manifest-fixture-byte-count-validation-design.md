# CLI Workload Replay Manifest Fixture Byte Count Validation Design

## Problem

`replay-workload --require-manifest` validates fixture paths and fingerprints, but the workload bundle manifest also records fixture byte counts. Those byte-count fields can currently drift from the archived fixture files while replay still succeeds.

## Desired Behavior

When `--require-manifest` is set, replay validates that manifest `workload_artifacts.cells_bytes` and `workload_artifacts.checkout_request_bytes` exactly match the byte lengths of `workload-cells.json` and `checkout-request.json`.

## Scope

- Reject self-inconsistent workload bundle manifests before replaying fixtures.
- Preserve existing path, fingerprint, artifact-directory, report-path, and workload-summary validation.
- Keep validation deterministic and local to the required manifest validation path.

## Out of Scope

- Changing the fingerprint algorithm.
- Validating replay output bundle byte counts.
- Changing workload bundle format version.
