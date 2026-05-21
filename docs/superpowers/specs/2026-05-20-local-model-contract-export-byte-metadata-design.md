# Local Model Contract Export Byte Metadata Design

## Problem

`benchmark-local-model --contract-dir` now reports durable `schema_bytes` and `grammar_bytes`, but the standalone `local-model-contract` export command still reports only paths and fingerprints. Operators using direct contract export cannot verify archived contract file sizes without computing them separately.

## Goal

Add schema and grammar byte-count metadata to the standalone local-model contract export JSON so all contract export surfaces expose the same basic integrity evidence.

## Scope

- Add `schema_bytes` and `grammar_bytes` to `local-model-contract` stdout JSON.
- Compute the byte counts from the exact schema and grammar strings written to disk.
- Keep existing paths, fingerprints, schema version, and file contents unchanged.

## Non-Goals

- Do not change benchmark contract artifact validation.
- Do not add JSON Schema or GBNF syntax validation in this slice.
- Do not change local model contract file names.

## Verification

- Extend the existing `cli_local_model_contract_writes_schema_and_grammar` CLI test to assert `schema_bytes` and `grammar_bytes` match the exported files.
- Run the focused local-model contract test and the full workspace verification gate.
