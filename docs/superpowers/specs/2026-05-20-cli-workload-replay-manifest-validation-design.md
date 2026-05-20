# CLI Workload Replay Manifest Validation Design

## Goal

Add an explicit replay gate that validates archived workload fixture files against `continuitydb-workload.manifest.json` before replay.

## Rationale

Storage-engine trials depend on immutable, reproducible fixture bundles. `replay-workload` already consumes `workload-cells.json` and `checkout-request.json`, but without manifest validation a tampered fixture can still be replayed as if it were the original archived workload.

## Behavior

- Add `replay-workload --require-manifest`.
- Require `continuitydb-workload.manifest.json` in `--artifact-dir` when the flag is set.
- Require manifest format `continuitydb.workload.bundle` with `format_version` 1.
- Compare the manifest `workload_artifacts.cells_fingerprint` with the current `workload-cells.json` FNV-1a fingerprint.
- Compare the manifest `workload_artifacts.checkout_request_fingerprint` with the current `checkout-request.json` FNV-1a fingerprint.
- Reject mismatches before parsing and replaying the workload.
- Preserve default replay behavior for hand-authored fixture directories unless the caller opts into manifest enforcement.

## Non-Goals

- Do not make manifest validation mandatory for all replay calls yet.
- Do not change workload bundle format version.
- Do not validate archived performance counts; `--compare-report` remains responsible for report comparison.
