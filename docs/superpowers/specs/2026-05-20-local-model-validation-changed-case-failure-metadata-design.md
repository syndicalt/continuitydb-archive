# Local Model Validation Changed-Case Failure Metadata Design

## Problem

`validate-local-model-bundle --failure-report-path` reports manifest and benchmark report evidence, but leaves `changed_case_report` null even when the rejected bundle contains `changed-cases.json`. CI artifacts should identify archived changed-case drift evidence involved in a rejected local Steward benchmark bundle.

## Goal

Add best-effort changed-case report metadata to local-model bundle validation failure reports.

## Scope

- Read `changed-cases.json` when present.
- Record report path, fingerprint, and byte count.
- Return `null` when the changed-case report does not exist or cannot be read.
- Preserve existing validation failure semantics and stderr behavior.

## Non-Goals

- Do not add response artifact manifest metadata in this slice.
- Do not validate changed-case report content from the failure metadata helper.
- Do not change successful validation output.

## Verification

- Add a CLI test that creates a changed-case local-model bundle, tampers manifest benchmark report bytes, and runs `validate-local-model-bundle --failure-report-path`.
- Assert the failure report includes `changed_case_report` path, fingerprint, and byte count.
- Run focused local-model CLI test and the full workspace verification gate.
