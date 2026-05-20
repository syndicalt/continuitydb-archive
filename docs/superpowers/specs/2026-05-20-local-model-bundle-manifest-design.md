# Local Model Bundle Manifest Design

## Goal

Add a top-level manifest to `benchmark-local-model --artifact-dir` output so archived local Steward benchmark bundles are self-describing from one stable file.

## Context

`benchmark-local-model --artifact-dir` currently writes contracts, prompts, real-run responses, a nested response manifest, and `benchmark-report.json`. The directory can be archived, but consumers must know the file naming convention and inspect the report to discover the bundle contents.

## Design

When `--artifact-dir <DIR>` is provided, the CLI writes `<DIR>/local-model-benchmark.manifest.json` after the benchmark or dry-run JSON has been assembled and the benchmark report has been written. The manifest uses a versioned format:

```json
{
  "format": "continuitydb.local_model.benchmark_bundle",
  "format_version": 1,
  "benchmark_report_path": ".../benchmark-report.json",
  "contract_artifacts": { "...": "..." },
  "prompt_artifacts": [{ "...": "..." }],
  "response_artifacts": [{ "...": "..." }],
  "response_artifact_manifest": { "...": "..." }
}
```

Dry-runs include contracts and prompts, an empty response artifact list, and a null response artifact manifest. Real runs include responses and the nested response manifest when response capture is enabled through the artifact directory.

The command stdout and `benchmark-report.json` include a `bundle_manifest` field containing the manifest path, fingerprint, and byte count. The manifest itself does not include its own fingerprint to avoid recursive content.

## Boundaries

- No new CLI flag is needed.
- Explicit per-artifact overrides still control where contracts, prompts, and responses are written.
- The bundle manifest exists only for `--artifact-dir`.
- Baseline records remain compact and do not store artifact paths or raw response content.

## Testing

Extend the existing real-run and dry-run artifact directory CLI tests. They should fail before implementation because the top-level manifest file and `bundle_manifest` JSON field do not exist. After implementation, tests verify the manifest format, report path, artifact counts, nested response manifest behavior, and fingerprint metadata.
