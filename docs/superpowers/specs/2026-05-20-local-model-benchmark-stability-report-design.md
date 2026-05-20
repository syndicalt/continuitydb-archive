# Local Model Benchmark Stability Report Design

## Problem

The local Steward benchmark could score a single model output, but it could not detect unstable output across repeated low-temperature runs. Pass/fail evaluation alone is insufficient because two different proposal rationales can both satisfy deterministic case requirements while still indicating nondeterministic model behavior.

## Goal

Add a public benchmark stability report API that re-runs the same benchmark suite multiple times and compares deterministic fingerprints of decoded proposal output per case.

The report should expose:

- number of trials
- suite-level stability
- per-case stability
- proposal fingerprints per trial
- one-based changed trial numbers for unstable cases

## Design

`LocalModelBenchmark::run_stability(identity, trials)` runs each suite case at least once, using `trials.max(1)`.

For each case and trial:

1. Invoke the existing local model Steward path.
2. Decode model output into proposals.
3. Build a deterministic proposal-output fingerprint that excludes random proposal IDs but includes action, rationale, citations, and proposal creation time.
4. Compare every trial fingerprint with the first trial fingerprint.

The API does not change normal benchmark recording, baseline compatibility, response schema, grammar, or policy semantics.

## Acceptance Criteria

- Repeated identical decoded proposal output reports stable.
- Different decoded proposals that both pass evaluation report unstable.
- Per-case reports expose changed trial numbers and proposal fingerprints.
- Normal `LocalModelBenchmark::run` behavior remains unchanged.
