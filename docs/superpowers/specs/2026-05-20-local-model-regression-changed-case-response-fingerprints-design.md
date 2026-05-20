# Local Model Regression Changed-Case Response Fingerprints Design

## Problem

Changed-case regression summaries explain which failure codes moved, but operators still need to cross-reference top-level response fingerprints to identify the raw model output behind a changed case. CI artifacts should make that link directly at the case-summary level.

## Design

- Reuse durable `LocalModelResponseFingerprint` metadata already stored on benchmark baselines.
- Add previous and current response fingerprint fields to `LocalModelBenchmarkCaseSummary`.
- Add previous and current response byte-count fields so missing or empty captures remain visible.
- Populate these fields by matching response fingerprints to changed evaluation cases by case name.
- Keep fields optional/defaulted for tolerant deserialization of older serialized regression summaries.
- Surface the new fields through existing CLI `baseline_comparison.changed_case_summaries` JSON.

## Test

- Extend the Steward regression summary test to assert changed cases carry distinct previous/current `fnv1a64` response fingerprints and nonzero byte counts.
- Extend the CLI same-outcome changed-case test to assert the public JSON exposes those response fingerprints and byte counts.
