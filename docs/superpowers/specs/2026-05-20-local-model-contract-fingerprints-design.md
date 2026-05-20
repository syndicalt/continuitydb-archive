# Local Model Contract Fingerprints Design

## Problem

ContinuityDB can export the local Steward model JSON Schema and GBNF grammar, and `benchmark-local-model --dry-run` can show candidate/runtime/suite metadata. The artifacts still lack deterministic fingerprints for the exact schema and grammar text. Operators need those fingerprints to connect archived contract files, preflight output, and later benchmark baselines.

## Goal

Expose deterministic fingerprints for the local model JSON Schema and GBNF grammar in CLI contract export and benchmark dry-run output.

## Non-Goals

- Do not change the schema or grammar content.
- Do not add cryptographic signing.
- Do not change local model benchmark scoring or baseline compatibility.
- Do not add model execution behavior.

## Design

Add a small CLI-local deterministic fingerprint helper for static contract text:

- algorithm: FNV-1a 64-bit;
- output format: `fnv1a64:<16 lowercase hex chars>`;
- input: raw UTF-8 bytes of the schema or grammar string.

Add fields:

- `schema_fingerprint`
- `grammar_fingerprint`

to:

- `local-model-contract` JSON output;
- `benchmark-local-model --dry-run` JSON output.

The helper is CLI-local because the current need is operator artifact integrity. A later library API can promote contract fingerprints if embedders need the same metadata without shelling out.

## Acceptance Criteria

- `local-model-contract` output includes schema and grammar fingerprints.
- The schema fingerprint matches the written schema file content.
- The grammar fingerprint matches the written grammar file content.
- `benchmark-local-model --dry-run` includes the same schema and grammar fingerprints.
- Existing benchmark execution and baseline JSON remain unchanged.
