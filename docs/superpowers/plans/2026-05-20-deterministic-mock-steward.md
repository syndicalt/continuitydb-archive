# Deterministic Mock Steward Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a deterministic `MockSteward` to `continuitydb-steward` that emits `StewardProposal` values from explicit rules without model inference or database mutation.

**Architecture:** The mock lives inside `continuitydb-steward` as a focused module. It reuses existing `StewardProposal`, `StewardAction`, `StewardIdentity`, `ProposalPolicy`, and `ProposalLedger` types. It does not add dependencies, does not apply proposal actions, and does not evaluate or record proposals itself.

**Tech Stack:** Rust 2021 workspace, existing `continuitydb-steward` crate, `chrono`, existing workspace lints, `cargo test`, `cargo fmt`, and `cargo clippy`.

---

## Files And Responsibilities

- Modify `crates/continuitydb-steward/src/lib.rs`: export mock types and add TDD coverage.
- Create `crates/continuitydb-steward/src/mock.rs`: `MockSteward`, `MockStewardInput`, and `MockStewardRule`.
- Modify `README.md`: include the deterministic mock Steward in current scope.
- Modify `docs/roadmap.md`: mark Steward milestone 4 implemented.

## Task 1: Mock Steward Emission

**Files:**
- Modify: `crates/continuitydb-steward/src/lib.rs`
- Create: `crates/continuitydb-steward/src/mock.rs`

- [ ] **Step 1: Write failing mock tests**

Modify `crates/continuitydb-steward/src/lib.rs`:

```rust
//! Deterministic Steward proposal substrate.

mod error;
mod ledger;
mod mock;
mod policy;
mod proposal;

pub use error::StewardError;
pub use ledger::{ProposalAuditRecord, ProposalLedger};
pub use mock::{MockSteward, MockStewardInput, MockStewardRule};
pub use policy::{ProposalDecision, ProposalOutcome, ProposalPolicy};
pub use proposal::{ProposalId, StewardAction, StewardIdentity, StewardProposal};
```

Add these tests to the existing `#[cfg(test)] mod tests` block:

```rust
#[test]
fn mock_steward_returns_no_proposals_for_empty_input() -> Result<(), Box<dyn std::error::Error>> {
    let steward = MockSteward::new(steward()?);
    let proposals = steward.propose(MockStewardInput::new(created_at()))?;

    assert!(proposals.is_empty());
    Ok(())
}

#[test]
fn mock_steward_emits_mark_frontier_with_identity() -> Result<(), Box<dyn std::error::Error>> {
    let identity = steward()?;
    let cell_id = StateCellId::new();
    let mock = MockSteward::new(identity.clone());
    let proposals = mock.propose(
        MockStewardInput::new(created_at()).with_rule(MockStewardRule::MarkFrontier {
            cell_id,
            rationale: "Evidence is stale and high impact.".to_string(),
            citations: vec!["test://frontier".to_string()],
        }),
    )?;

    assert_eq!(proposals.len(), 1);
    assert_eq!(proposals[0].steward(), &identity);
    assert!(matches!(
        proposals[0].action(),
        StewardAction::MarkFrontier { cell_id: actual } if *actual == cell_id
    ));
    assert_eq!(proposals[0].citations(), &["test://frontier".to_string()]);
    Ok(())
}

#[test]
fn mock_steward_emits_link_revision_preserving_fields() -> Result<(), Box<dyn std::error::Error>> {
    let source = StateCellId::new();
    let target = StateCellId::new();
    let mock = MockSteward::new(steward()?);
    let proposals = mock.propose(
        MockStewardInput::new(created_at()).with_rule(MockStewardRule::LinkRevision {
            source,
            kind: RevisionLinkKind::ConflictsWith,
            target,
            rationale: "The two cells make incompatible claims.".to_string(),
            citations: vec!["test://conflict".to_string()],
        }),
    )?;

    assert!(matches!(
        proposals[0].action(),
        StewardAction::LinkRevision {
            source: actual_source,
            kind: RevisionLinkKind::ConflictsWith,
            target: actual_target,
        } if *actual_source == source && *actual_target == target
    ));
    Ok(())
}

#[test]
fn mock_steward_preserves_rule_order() -> Result<(), Box<dyn std::error::Error>> {
    let first = StateCellId::new();
    let second = StateCellId::new();
    let mock = MockSteward::new(steward()?);
    let proposals = mock.propose(
        MockStewardInput::new(created_at())
            .with_rule(MockStewardRule::MarkFrontier {
                cell_id: first,
                rationale: "First frontier proposal.".to_string(),
                citations: vec!["test://first".to_string()],
            })
            .with_rule(MockStewardRule::RequestVerification {
                cell_id: Some(second),
                request: "Verify the second cell.".to_string(),
                rationale: "Second verification proposal.".to_string(),
                citations: vec!["test://second".to_string()],
            }),
    )?;

    assert!(matches!(
        proposals[0].action(),
        StewardAction::MarkFrontier { cell_id } if *cell_id == first
    ));
    assert!(matches!(
        proposals[1].action(),
        StewardAction::RequestVerification { cell_id, request }
            if *cell_id == Some(second) && request == "Verify the second cell."
    ));
    Ok(())
}

#[test]
fn mock_steward_reuses_proposal_constructor_validation() -> Result<(), Box<dyn std::error::Error>> {
    let mock = MockSteward::new(steward()?);
    let result = mock.propose(
        MockStewardInput::new(created_at()).with_rule(MockStewardRule::MarkFrontier {
            cell_id: StateCellId::new(),
            rationale: " ".to_string(),
            citations: vec!["test://frontier".to_string()],
        }),
    );

    assert!(matches!(result, Err(StewardError::EmptyRationale)));
    Ok(())
}
```

- [ ] **Step 2: Run tests to verify failure**

Run:

```bash
cargo test -p continuitydb-steward
```

Expected: FAIL because `mock.rs`, `MockSteward`, `MockStewardInput`, and `MockStewardRule` do not exist.

- [ ] **Step 3: Add mock implementation**

Create `crates/continuitydb-steward/src/mock.rs`:

```rust
//! Deterministic mock Steward.

use chrono::{DateTime, Utc};
use continuitydb_core::{SemanticAnchor, StateCellId};
use continuitydb_revision::RevisionLinkKind;

use crate::{StewardAction, StewardError, StewardIdentity, StewardProposal};

/// Deterministic Steward used for test-first development.
#[derive(Clone, Debug)]
pub struct MockSteward {
    identity: StewardIdentity,
}

impl MockSteward {
    /// Creates a mock Steward with a fixed identity.
    pub fn new(identity: StewardIdentity) -> Self {
        Self { identity }
    }

    /// Emits proposals from explicit deterministic rules.
    pub fn propose(
        &self,
        input: MockStewardInput,
    ) -> Result<Vec<StewardProposal>, StewardError> {
        input
            .rules
            .into_iter()
            .map(|rule| rule.into_proposal(self.identity.clone(), input.created_at))
            .collect()
    }
}

/// Input batch for deterministic mock proposal generation.
#[derive(Clone, Debug, Default)]
pub struct MockStewardInput {
    created_at: DateTime<Utc>,
    rules: Vec<MockStewardRule>,
}

impl MockStewardInput {
    /// Creates an empty input batch.
    pub fn new(created_at: DateTime<Utc>) -> Self {
        Self {
            created_at,
            rules: Vec::new(),
        }
    }

    /// Appends a deterministic rule to this input batch.
    pub fn with_rule(mut self, rule: MockStewardRule) -> Self {
        self.rules.push(rule);
        self
    }
}

/// Deterministic rule that maps to one Steward proposal action.
#[derive(Clone, Debug, PartialEq)]
pub enum MockStewardRule {
    /// Emits a `CreateCellDraft` proposal.
    CreateCellDraft {
        /// Semantic anchors for the draft.
        anchors: Vec<SemanticAnchor>,
        /// Draft payload text.
        payload_text: String,
        /// Proposal rationale.
        rationale: String,
        /// Supporting citation locators.
        citations: Vec<String>,
    },
    /// Emits a `LinkRevision` proposal.
    LinkRevision {
        /// Source StateCell version.
        source: StateCellId,
        /// Revision link kind.
        kind: RevisionLinkKind,
        /// Target StateCell version.
        target: StateCellId,
        /// Proposal rationale.
        rationale: String,
        /// Supporting citation locators.
        citations: Vec<String>,
    },
    /// Emits an `AdjustConfidence` proposal.
    AdjustConfidence {
        /// Target StateCell version.
        cell_id: StateCellId,
        /// Proposed confidence.
        proposed_confidence: f32,
        /// Proposal rationale.
        rationale: String,
        /// Supporting citation locators.
        citations: Vec<String>,
    },
    /// Emits a `LabelAnswerability` proposal.
    LabelAnswerability {
        /// Target StateCell version.
        cell_id: StateCellId,
        /// Proposed answerability questions.
        questions: Vec<String>,
        /// Proposal rationale.
        rationale: String,
        /// Supporting citation locators.
        citations: Vec<String>,
    },
    /// Emits a `MarkFrontier` proposal.
    MarkFrontier {
        /// Target StateCell version.
        cell_id: StateCellId,
        /// Proposal rationale.
        rationale: String,
        /// Supporting citation locators.
        citations: Vec<String>,
    },
    /// Emits a `RequestVerification` proposal.
    RequestVerification {
        /// Target StateCell version when cell-specific.
        cell_id: Option<StateCellId>,
        /// Verification request text.
        request: String,
        /// Proposal rationale.
        rationale: String,
        /// Supporting citation locators.
        citations: Vec<String>,
    },
}

impl MockStewardRule {
    fn into_proposal(
        self,
        steward: StewardIdentity,
        created_at: DateTime<Utc>,
    ) -> Result<StewardProposal, StewardError> {
        let (action, rationale, citations) = match self {
            Self::CreateCellDraft {
                anchors,
                payload_text,
                rationale,
                citations,
            } => (
                StewardAction::CreateCellDraft {
                    anchors,
                    payload_text,
                },
                rationale,
                citations,
            ),
            Self::LinkRevision {
                source,
                kind,
                target,
                rationale,
                citations,
            } => (
                StewardAction::LinkRevision {
                    source,
                    kind,
                    target,
                },
                rationale,
                citations,
            ),
            Self::AdjustConfidence {
                cell_id,
                proposed_confidence,
                rationale,
                citations,
            } => (
                StewardAction::AdjustConfidence {
                    cell_id,
                    proposed_confidence,
                },
                rationale,
                citations,
            ),
            Self::LabelAnswerability {
                cell_id,
                questions,
                rationale,
                citations,
            } => (
                StewardAction::LabelAnswerability { cell_id, questions },
                rationale,
                citations,
            ),
            Self::MarkFrontier {
                cell_id,
                rationale,
                citations,
            } => (StewardAction::MarkFrontier { cell_id }, rationale, citations),
            Self::RequestVerification {
                cell_id,
                request,
                rationale,
                citations,
            } => (
                StewardAction::RequestVerification { cell_id, request },
                rationale,
                citations,
            ),
        };

        StewardProposal::new(
            crate::ProposalId::new(),
            steward,
            action,
            rationale,
            citations,
            created_at,
        )
    }
}
```

- [ ] **Step 4: Run mock tests**

Run:

```bash
cargo test -p continuitydb-steward
```

Expected: PASS for all Steward tests.

- [ ] **Step 5: Commit mock Steward**

Run:

```bash
git add crates/continuitydb-steward
git commit -m "feat: add deterministic mock steward"
```

## Task 2: Policy And Ledger Compatibility

**Files:**
- Modify: `crates/continuitydb-steward/src/lib.rs`

- [ ] **Step 1: Write failing compatibility tests**

Add these tests to the existing `#[cfg(test)] mod tests` block:

```rust
#[test]
fn mock_steward_output_can_be_evaluated_by_policy() -> Result<(), Box<dyn std::error::Error>> {
    let mock = MockSteward::new(steward()?);
    let proposals = mock.propose(
        MockStewardInput::new(created_at()).with_rule(MockStewardRule::MarkFrontier {
            cell_id: StateCellId::new(),
            rationale: "Evidence is stale and high impact.".to_string(),
            citations: vec!["test://frontier".to_string()],
        }),
    )?;

    let decision = ProposalPolicy::strict().evaluate(&proposals[0], created_at());

    assert_eq!(decision.outcome(), ProposalOutcome::Accepted);
    Ok(())
}

#[test]
fn mock_steward_output_can_be_recorded_in_ledger() -> Result<(), Box<dyn std::error::Error>> {
    let mock = MockSteward::new(steward()?);
    let proposals = mock.propose(
        MockStewardInput::new(created_at()).with_rule(MockStewardRule::RequestVerification {
            cell_id: None,
            request: "Verify the source evidence.".to_string(),
            rationale: "The evidence requires manual review.".to_string(),
            citations: vec!["test://verification".to_string()],
        }),
    )?;
    let decision = ProposalPolicy::strict().evaluate(&proposals[0], created_at());
    let proposal_id = proposals[0].id();
    let mut ledger = ProposalLedger::default();

    ledger.record(proposals[0].clone(), decision)?;

    assert_eq!(
        ledger.record_by_id(proposal_id).map(|record| record.proposal().id()),
        Some(proposal_id)
    );
    Ok(())
}
```

- [ ] **Step 2: Run compatibility tests**

Run:

```bash
cargo test -p continuitydb-steward
```

Expected: PASS. If this fails, fix `mock.rs` rather than weakening tests.

- [ ] **Step 3: Commit compatibility tests**

Run:

```bash
git add crates/continuitydb-steward
git commit -m "test: cover mock steward policy and ledger compatibility"
```

If the tests pass without implementation changes, this is still a valid test-only commit because it covers spec acceptance criteria.

## Task 3: Roadmap And Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [ ] **Step 1: Update README current scope**

Modify `README.md` current scope list to include:

```markdown
- Deterministic mock Steward for test-first development.
```

- [ ] **Step 2: Update roadmap milestone status**

Modify `docs/roadmap.md` under `## Steward Milestones`:

```markdown
4. Build a deterministic mock steward for test-first development. Implemented in `continuitydb-steward`.
```

- [ ] **Step 3: Run formatter**

Run:

```bash
cargo fmt --all -- --check
```

Expected: PASS.

- [ ] **Step 4: Run clippy**

Run:

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: PASS with zero warnings.

- [ ] **Step 5: Run tests**

Run:

```bash
cargo test --workspace
```

Expected: PASS.

- [ ] **Step 6: Commit docs and verification fixes**

Run:

```bash
git add README.md docs/roadmap.md crates/continuitydb-steward
git commit -m "docs: mark mock steward roadmap progress"
```

If there are no doc or verification fixes after Steps 1-5, do not create an empty commit.

## Self-Review Against Spec

- `MockSteward`, `MockStewardInput`, and `MockStewardRule` are public: Task 1.
- No model inference dependencies: Task 1 adds no manifest dependency.
- Empty input returns no proposals: Task 1 test.
- Mark-frontier proposal includes identity: Task 1 test.
- Link-revision proposal preserves fields: Task 1 test.
- Multiple rules preserve order: Task 1 test.
- Invalid citations or rationale return `StewardError`: Task 1 test.
- Mock output can be evaluated by `ProposalPolicy::strict`: Task 2 test.
- Mock output can be recorded in `ProposalLedger`: Task 2 test.
- Workspace verification: Task 3.

## Execution Notes

This plan intentionally avoids model inference, async workers, storage persistence, frontier/watch scheduling, and applying accepted proposals to core database state. Those are later roadmap slices.
