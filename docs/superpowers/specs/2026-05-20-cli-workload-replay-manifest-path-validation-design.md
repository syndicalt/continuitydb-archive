# CLI Workload Replay Manifest Path Validation Design

## Goal

Reject workload replay bundles whose manifest records fixture paths that do not match the artifact directory being replayed.

## Rationale

`replay-workload --require-manifest` already validates fixture fingerprints, but a manifest can still be self-inconsistent if its paths point somewhere other than the replayed `workload-cells.json` and `checkout-request.json`. Storage-engine replay artifacts should validate both identity and location metadata.

## Behavior

- Validate `workload_artifacts.cells_path` equals `<artifact-dir>/workload-cells.json`.
- Validate `workload_artifacts.checkout_request_path` equals `<artifact-dir>/checkout-request.json`.
- Reject mismatches before parsing workload cells or checkout requests.
- Reuse existing input-manifest validation failure reporting and replay bundle behavior.

## Non-Goals

- Do not support relocated manifests with relative paths yet.
- Do not change workload bundle format version.
- Do not make manifest validation mandatory for all replays.
