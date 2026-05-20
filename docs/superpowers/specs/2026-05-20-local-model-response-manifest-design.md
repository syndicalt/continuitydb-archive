# Local Model Response Manifest Design

## Goal

Make `benchmark-local-model --response-dir` output self-describing by writing a response artifact manifest into the selected directory.

## Context

The CLI can write one raw model response file per evaluation case and report artifact metadata on stdout or through `--report-path`. If the response directory is archived without the command output, the raw files lose their benchmark artifact index. The directory should include its own manifest so real local model trials can be inspected later.

## Design

When a real benchmark run uses `--response-dir`, write `local-model-responses.manifest.json` inside that directory. The manifest contains:

- `format`
- `format_version`
- `artifacts`

Each artifact entry mirrors the public response artifact metadata:

- `case_name`
- `captured`
- `response_path`
- `response_fingerprint`
- `response_bytes`

The benchmark JSON includes `response_artifact_manifest` with:

- `manifest_path`
- `manifest_fingerprint`
- `manifest_bytes`

Dry-runs do not execute the model and do not write a manifest, so they return `response_artifact_manifest = null`.

## Boundaries

This does not change baseline compatibility, response scoring, raw response contents, or the durable response fingerprint summaries. It only makes the optional response artifact directory independently auditable.

## Documentation

Add README and roadmap entries for CLI local-model response artifact manifests.
