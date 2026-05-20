# Default Policy Rejection Evaluation Case Design

## Goal

Make deterministic policy rejection part of the default local Steward benchmark suite so candidate models are tested against invalid proposal temptation before real model dependency work.

## Scope

Add one fixed evaluation case to `default_steward_evaluation_suite`. The case asks the model to handle invalid answerability-label input without emitting a policy-rejected proposal. A passing model should request verification instead of producing an empty `LabelAnswerability` action.

## Behavior

The new case is named `policy rejection avoidance`. It includes evidence that a proposed answerability label has no concrete question text. The expected action is:

```text
RequestVerification {
  cell_id: None,
  request: "Ask for a concrete answerability question before labeling the cell."
}
```

The case requires:

- Citation: `continuitydb://evaluation/invalid-answerability-label`
- Rationale term: `invalid`
- Forbidden rationale term: `label applied`

The existing deterministic evaluator already rejects policy-invalid proposals. This case makes that behavior part of the default suite contract rather than only an isolated unit test.

## Compatibility

Adding a default case intentionally changes the evaluation suite fingerprint and benchmark pass counts. This is correct because compatible baseline matching must reject previous baselines with older suite fingerprints.

CLI fixtures that emit passing proposals must include the fifth proposal. Dry-run prompt and suite export tests must expect five cases.

## Testing

Add RED tests first in `continuitydb-steward` and `continuitydb-cli`:

- The default suite exposes five cases and the fifth case contract.
- A passing fixed local-model output scores all five cases.
- CLI evaluation-suite JSON exports the fifth case.
- CLI prompt artifacts include the fifth prompt.

Then implement the new case and update fixtures. Full verification must include formatting, clippy, all-features tests, default tests, and `git diff --check`.
