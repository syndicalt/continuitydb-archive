# Local Model Contract Byte Metadata Design

## Problem

Local-model benchmark contract artifacts record schema and grammar paths plus fingerprints, but they do not record byte counts. Bundle validation now checks fingerprints and failure reports preserve current byte counts, yet durable benchmark reports and bundle manifests cannot independently identify contract artifact sizes.

## Goal

Add durable schema and grammar byte-count metadata to local-model contract artifacts and validate those byte counts when archived benchmark bundles are checked.

## Scope

- Add `schema_bytes` and `grammar_bytes` to `contract_artifacts` in `benchmark-local-model` JSON.
- Preserve those fields in `local-model-benchmark.manifest.json` through the existing `contract_artifacts` projection.
- Validate schema and grammar byte counts in `validate-local-model-bundle`.
- Reject byte-count mismatches with deterministic diagnostics.

## Non-Goals

- Do not change the contract file names or contents.
- Do not validate JSON Schema or GBNF syntax in this slice.
- Do not introduce a new manifest format version.

## Verification

- Add a CLI test/assertion that `benchmark-local-model --contract-dir` reports `schema_bytes` and `grammar_bytes` matching files on disk.
- Add a CLI test that tampers `local-model-benchmark.manifest.json` contract byte metadata and asserts `validate-local-model-bundle` rejects the bundle.
- Run focused local-model CLI tests and the full workspace verification gate.
