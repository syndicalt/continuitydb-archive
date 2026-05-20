# CLI Local Model Changed-Case Metadata Design

## Problem

Changed-case reports are now archiveable, but benchmark stdout and bundle manifests only expose a path. Other ContinuityDB artifacts expose fingerprints and byte counts so CI can verify archived files without reparsing every artifact.

## Design

- Add changed-case artifact metadata with path, FNV-1a fingerprint, and byte count.
- Keep the existing `changed_case_report_path` field for compatibility.
- Add a new `changed_case_report` object to full benchmark JSON and bundle manifests.
- Emit `null` metadata when no changed-case report is written.

## Test

- Extend a feature-gated CLI artifact-bundle test to assert `changed_case_report.report_path`, `changed_case_report.report_fingerprint`, and `changed_case_report.report_bytes` appear in stdout and `local-model-benchmark.manifest.json`.
