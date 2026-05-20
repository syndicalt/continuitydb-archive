# Local Model Typed Failure Diagnostics Design

## Problem

Local Steward model benchmark reports currently collapse backend execution failures and invalid model responses into one `ModelError` evaluation failure. That makes CI artifacts less actionable: operators cannot tell whether the executable failed to run or whether the model produced undecodable output.

## Constraints

- Preserve deterministic evaluation semantics.
- Keep local models proposal-only.
- Do not expose stderr or environment-specific process details in durable benchmark reports.
- Keep the legacy generic `ModelError` variant available for compatibility.
- Add typed failure variants that serialize through existing report JSON paths.

## Test

Add Steward evaluation tests that prove:

- A backend execution error becomes `ModelExecutionFailed`.
- An invalid raw response becomes `InvalidModelResponse`.
