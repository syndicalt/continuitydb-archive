# Local Model Validation Manifest Failure Metadata Design

## Problem

`validate-local-model-bundle --failure-report-path` now writes structured failure reports, but `manifest` is always `null`. When a bundle is rejected, CI artifacts should identify the exact `local-model-benchmark.manifest.json` bytes that were inspected.

## Goal

Add best-effort local-model benchmark manifest metadata to validation failure reports.

## Scope

- Read `local-model-benchmark.manifest.json` if it exists.
- Record manifest path, fingerprint, and byte count.
- Return `null` when the manifest cannot be read.
- Preserve existing validation failure semantics and stderr behavior.

## Non-Goals

- Do not add benchmark report metadata in this slice.
- Do not validate or parse the manifest in the failure metadata helper.
- Do not change success validation output.

## Verification

- Add a CLI test that tampers manifest report byte count, runs `validate-local-model-bundle --failure-report-path`, and asserts failure report manifest metadata is populated.
- Run the focused local-model CLI test.
- Run the full workspace verification gate.
