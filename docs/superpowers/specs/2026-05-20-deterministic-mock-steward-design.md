# Deterministic Mock Steward Design

## Status

Draft for review. This spec covers the next frontier roadmap slice after the Steward proposal substrate.

## Goal

Add a deterministic mock Steward that emits valid `StewardProposal` values without invoking any model. The mock Steward gives ContinuityDB a stable test harness for future model-backed stewardship, proposal quality evaluation, and frontier/watch integration.

## Context

The current `continuitydb-steward` crate already provides:

- `StewardProposal`
- `StewardAction`
- `StewardIdentity`
- `ProposalPolicy`
- `ProposalDecision`
- `ProposalLedger`

The next roadmap item is:

> Build a deterministic mock steward for test-first development.

The mock Steward must use the existing proposal substrate. It should not introduce model inference, async workers, storage persistence, or direct mutations to StateCells, revision graphs, or kernels.

## Non-Goals

- Do not add Qwen, llama.cpp, mistral.rs, or any model dependency.
- Do not add stochastic behavior.
- Do not add background scheduling or watch loops.
- Do not apply accepted proposals to core data structures.
- Do not infer from raw text using heuristic NLP. The mock is a deterministic harness, not a fake LLM.

## Architecture

Add mock Steward types to `continuitydb-steward`:

```text
continuitydb-steward
  StewardProposal substrate
  ProposalPolicy
  ProposalLedger
  MockSteward
  MockStewardInput
  MockStewardRule
```

The mock should produce proposals from explicit rule inputs. It should make later tests able to say:

```rust
let steward = MockSteward::new(StewardIdentity::new("deterministic-mock", "0.1.0", "strict")?);
let proposals = steward.propose(MockStewardInput::for_cell(cell_id)
    .with_rule(MockStewardRule::MarkFrontier { citation, rationale }));
```

## Public Types

### MockSteward

Owns a `StewardIdentity` and emits proposals from deterministic inputs.

Responsibilities:

- Attach the configured `StewardIdentity` to every proposal.
- Use caller-provided citations and rationale.
- Emit proposals in input rule order.
- Generate fresh proposal IDs.
- Use caller-provided `created_at` for deterministic tests.

### MockStewardInput

Represents one deterministic proposal request batch.

Fields:

- `created_at`: UTC timestamp used for every emitted proposal.
- `rules`: ordered list of `MockStewardRule`.

### MockStewardRule

Represents a deterministic rule that maps to one `StewardAction`.

Initial variants:

- `CreateCellDraft`
- `LinkRevision`
- `AdjustConfidence`
- `LabelAnswerability`
- `MarkFrontier`
- `RequestVerification`

Each rule must include:

- action-specific data
- `rationale`
- `citations`

This duplication is intentional. Tests should be explicit about why each proposal exists and what evidence supports it.

## Behavior

`MockSteward::propose(input)` returns `Result<Vec<StewardProposal>, StewardError>`.

Rules:

- Empty input produces an empty proposal list.
- Rule order is preserved.
- Invalid rule rationale or citations fail through `StewardProposal::new`.
- The mock does not run `ProposalPolicy`; callers can evaluate proposals separately.
- The mock never records to `ProposalLedger`; callers own recording.
- The mock never applies proposal actions.

## Testing Requirements

Implementation must be test-first and cover:

- Empty input returns no proposals.
- Mark-frontier rule emits a `MarkFrontier` proposal with the mock identity.
- Link-revision rule emits a `LinkRevision` proposal preserving source, target, and link kind.
- Multiple rules preserve order.
- Invalid citations or rationale return the existing `StewardError`.
- Mock output can be evaluated by `ProposalPolicy::strict`.
- Mock output can be recorded in `ProposalLedger`.

## Future Integration

After this slice, later roadmap items can use `MockSteward` to test:

- storage-backed proposal persistence
- conversion from accepted proposal intents to revision/cell operations
- frontier/watch integration
- local model steward API conformance
- Qwen2.5 proposal quality tests against the same output schema

## Acceptance Criteria

- `MockSteward`, `MockStewardInput`, and `MockStewardRule` are public and documented.
- No model inference dependencies are introduced.
- The mock emits only `StewardProposal` objects and does not mutate database state.
- Tests cover deterministic emission, policy compatibility, and ledger compatibility.
- Workspace `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` pass.
