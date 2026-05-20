# Steward Proposal Substrate Design

## Status

Draft for review. This spec covers the first buildable frontier roadmap slice for the embedded database Steward.

## Goal

Build the deterministic proposal, policy, and audit substrate that future embedded Steward models must use before any model inference is added.

The Steward proposal substrate makes model-assisted database maintenance possible without letting a model directly mutate committed truth.

## Context

ContinuityDB's roadmap adds an embedded database Steward: a local model-assisted layer that helps maintain the epistemic health of the datastore. The Steward may eventually propose StateCell creation, supersession, conflicts, answerability labels, frontier work, and verification tasks.

The first implementation must not invoke a real model. It should define the typed boundary that model output will later be forced through.

## Non-Goals

- Do not add Qwen, llama.cpp, mistral.rs, or any real inference dependency.
- Do not add async runtimes or background workers.
- Do not let proposals mutate `StateCell`, `RevisionGraph`, or storage kernels directly.
- Do not build a general agent runtime.
- Do not hide rejected proposals. Rejections are audit artifacts.

## Architecture

Add a new workspace crate:

```text
crates/continuitydb-steward/
```

The crate owns deterministic Steward proposal semantics:

```text
continuitydb-steward
  StewardProposal
  StewardAction
  StewardIdentity
  ProposalPolicy
  ProposalDecision
  ProposalLedger
  ProposalAuditRecord
```

It may depend on:

- `continuitydb-core` for `StateCellId`, `SemanticAnchor`, `Confidence`, and evidence/citation types.
- `continuitydb-revision` for `RevisionLinkKind`.
- `chrono`, `serde`, `thiserror`, and `uuid`.

It must not depend on checkout, memory storage, CLI, or model inference crates.

## Data Model

### StewardIdentity

Identifies the proposing steward.

Fields:

- `name`: stable steward name, such as `deterministic-mock` or `qwen2.5-0.5b`.
- `version`: implementation or model version.
- `prompt_profile`: prompt or policy profile identifier.

### StewardProposal

Represents one model-assisted or deterministic proposal.

Fields:

- `id`: immutable proposal ID.
- `steward`: `StewardIdentity`.
- `action`: `StewardAction`.
- `rationale`: human-readable explanation.
- `citations`: source locators that support the proposal.
- `created_at`: UTC timestamp.

Required invariants:

- A proposal must include at least one citation.
- A proposal must include non-empty rationale.
- A proposal must include non-empty steward identity fields.

### StewardAction

Initial action variants:

- `CreateCellDraft`: proposes a new StateCell draft by semantic anchors and payload text.
- `LinkRevision`: proposes a revision link between two StateCell versions.
- `AdjustConfidence`: proposes a confidence value for an existing StateCell.
- `LabelAnswerability`: proposes answerability questions for an existing StateCell.
- `MarkFrontier`: proposes that a StateCell should enter frontier monitoring.
- `RequestVerification`: proposes human or automated verification work.

The first implementation should store action intent only. Applying action intent to core data structures is a later slice.

### ProposalPolicy

Validates proposals deterministically.

First policy rules:

- Reject missing citations.
- Reject empty rationale.
- Reject empty steward identity.
- Reject confidence adjustments outside `0.0..=1.0`.
- Reject answerability labels with no non-empty questions.
- Accept structurally valid proposals.

The policy returns a `ProposalDecision`, not a mutation.

### ProposalDecision

Represents policy output.

Fields:

- `proposal_id`
- `outcome`: `Accepted` or `Rejected`
- `reasons`: deterministic policy reasons
- `decided_at`

Rejections must include at least one reason. Accepted decisions may include reasons such as `policy:structurally-valid`.

### ProposalLedger

In-memory append-only audit ledger for proposal decisions.

Responsibilities:

- Record proposals with their decisions.
- Preserve both accepted and rejected proposals.
- Return audit records by proposal ID.
- Return all records in insertion order.

This is not the final persistence story. It is the deterministic substrate that future storage integration can persist.

## Public API Shape

```rust
let policy = ProposalPolicy::strict();
let decision = policy.evaluate(&proposal);

let mut ledger = ProposalLedger::default();
ledger.record(proposal, decision)?;

let audit_record = ledger.record_by_id(proposal_id);
```

The API must make it impossible to record a decision for a different proposal ID than the proposal being recorded.

## Testing Requirements

The implementation plan must be test-first and cover:

- Missing citations are rejected.
- Empty rationale is rejected.
- Empty steward identity is rejected.
- Invalid confidence adjustment is rejected.
- Empty answerability labels are rejected.
- Structurally valid `LinkRevision` proposals are accepted.
- Accepted proposals are auditable.
- Rejected proposals are auditable with reasons.
- Ledger rejects mismatched proposal/decision IDs.
- Ledger preserves insertion order.

## Future Integration

After this substrate exists, later roadmap slices can add:

1. Deterministic mock Steward.
2. Storage-backed proposal persistence.
3. Conversion from accepted proposal intents to revision/cell operations.
4. Frontier/watch integration.
5. Feature-gated local model inference.
6. Qwen2.5-0.5B-Instruct proposal quality evaluation.

## Acceptance Criteria

- `continuitydb-steward` exists as a workspace crate.
- The crate has no model inference dependencies.
- Proposal, policy, decision, and ledger types are public and documented.
- Tests cover both acceptance and rejection paths.
- Workspace `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` pass.
