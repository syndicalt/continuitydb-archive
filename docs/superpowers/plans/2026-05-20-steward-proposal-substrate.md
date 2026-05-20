# Steward Proposal Substrate Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the deterministic `continuitydb-steward` crate with Steward proposal types, strict policy evaluation, proposal decisions, and an append-only audit ledger.

**Architecture:** `continuitydb-steward` is a pure semantic crate. It depends on `continuitydb-core` and `continuitydb-revision` for existing domain identifiers and revision link kinds, but it does not depend on model inference, storage backends, checkout, memory, or CLI crates. The crate records proposal intent and policy decisions only; it never mutates `StateCell`, `RevisionGraph`, or a storage kernel.

**Tech Stack:** Rust 2021 workspace, `chrono`, `serde`, `thiserror`, `uuid`, existing workspace lints, `cargo test`, `cargo fmt`, and `cargo clippy`.

---

## Files And Responsibilities

- Modify `Cargo.toml`: add `crates/continuitydb-steward` to workspace members.
- Create `crates/continuitydb-steward/Cargo.toml`: crate manifest with only deterministic dependencies.
- Create `crates/continuitydb-steward/src/lib.rs`: public exports and tests.
- Create `crates/continuitydb-steward/src/error.rs`: `StewardError` for constructor and ledger failures.
- Create `crates/continuitydb-steward/src/proposal.rs`: `ProposalId`, `StewardIdentity`, `StewardProposal`, and `StewardAction`.
- Create `crates/continuitydb-steward/src/policy.rs`: `ProposalPolicy`, `ProposalDecision`, `ProposalOutcome`, and policy reasons.
- Create `crates/continuitydb-steward/src/ledger.rs`: `ProposalAuditRecord` and `ProposalLedger`.

## Task 1: Crate Registration And Proposal Types

**Files:**
- Modify: `Cargo.toml`
- Create: `crates/continuitydb-steward/Cargo.toml`
- Create: `crates/continuitydb-steward/src/lib.rs`
- Create: `crates/continuitydb-steward/src/error.rs`
- Create: `crates/continuitydb-steward/src/proposal.rs`

- [ ] **Step 1: Write failing proposal tests**

Modify `Cargo.toml` workspace members:

```toml
members = [
    "crates/continuitydb-core",
    "crates/continuitydb-kernel",
    "crates/continuitydb-memory",
    "crates/continuitydb-revision",
    "crates/continuitydb-checkout",
    "crates/continuitydb-cli",
    "crates/continuitydb-steward",
]
```

Create `crates/continuitydb-steward/Cargo.toml`:

```toml
[package]
name = "continuitydb-steward"
version = "0.1.0"
edition.workspace = true
license.workspace = true
repository.workspace = true
rust-version.workspace = true

[dependencies]
chrono.workspace = true
continuitydb-core = { path = "../continuitydb-core" }
continuitydb-revision = { path = "../continuitydb-revision" }
serde.workspace = true
thiserror.workspace = true
uuid.workspace = true

[lints]
workspace = true
```

Create `crates/continuitydb-steward/src/lib.rs`:

```rust
//! Deterministic Steward proposal substrate.

mod error;
mod proposal;

pub use error::StewardError;
pub use proposal::{ProposalId, StewardAction, StewardIdentity, StewardProposal};

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use continuitydb_core::{SemanticAnchor, StateCellId};
    use continuitydb_revision::RevisionLinkKind;

    use super::{ProposalId, StewardAction, StewardError, StewardIdentity, StewardProposal};

    fn created_at() -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 5, 20, 0, 0, 0)
            .single()
            .unwrap_or_else(Utc::now)
    }

    #[test]
    fn steward_identity_rejects_empty_fields() {
        let result = StewardIdentity::new("", "0.1.0", "strict");

        assert!(matches!(result, Err(StewardError::EmptyStewardIdentity)));
    }

    #[test]
    fn proposal_requires_rationale_and_citations() -> Result<(), Box<dyn std::error::Error>> {
        let steward = StewardIdentity::new("deterministic-mock", "0.1.0", "strict")?;
        let action = StewardAction::MarkFrontier {
            cell_id: StateCellId::new(),
        };

        let no_rationale = StewardProposal::new(
            ProposalId::new(),
            steward.clone(),
            action.clone(),
            "",
            vec!["test://evidence".to_string()],
            created_at(),
        );
        let no_citations = StewardProposal::new(
            ProposalId::new(),
            steward,
            action,
            "Cell has stale evidence.",
            Vec::new(),
            created_at(),
        );

        assert!(matches!(no_rationale, Err(StewardError::EmptyRationale)));
        assert!(matches!(no_citations, Err(StewardError::MissingCitations)));
        Ok(())
    }

    #[test]
    fn proposal_carries_link_revision_intent() -> Result<(), Box<dyn std::error::Error>> {
        let source = StateCellId::new();
        let target = StateCellId::new();
        let proposal = StewardProposal::new(
            ProposalId::new(),
            StewardIdentity::new("deterministic-mock", "0.1.0", "strict")?,
            StewardAction::LinkRevision {
                source,
                kind: RevisionLinkKind::Supersedes,
                target,
            },
            "New evidence supersedes the prior cell.",
            vec!["test://evidence".to_string()],
            created_at(),
        )?;

        assert_eq!(proposal.citations(), &["test://evidence".to_string()]);
        assert!(matches!(
            proposal.action(),
            StewardAction::LinkRevision {
                source: actual_source,
                kind: RevisionLinkKind::Supersedes,
                target: actual_target
            } if *actual_source == source && *actual_target == target
        ));
        Ok(())
    }

    #[test]
    fn proposal_carries_confidence_and_answerability_intent() -> Result<(), Box<dyn std::error::Error>> {
        let cell_id = StateCellId::new();
        let confidence = StewardAction::AdjustConfidence {
            cell_id,
            proposed_confidence: 0.8,
        };
        let answerability = StewardAction::LabelAnswerability {
            cell_id,
            questions: vec!["what changed?".to_string()],
        };

        assert!(matches!(
            confidence,
            StewardAction::AdjustConfidence {
                proposed_confidence: actual,
                ..
            } if actual == 0.8
        ));
        assert!(matches!(
            answerability,
            StewardAction::LabelAnswerability { questions, .. } if questions == vec!["what changed?".to_string()]
        ));
        Ok(())
    }

    #[test]
    fn proposal_carries_create_cell_draft_intent() -> Result<(), Box<dyn std::error::Error>> {
        let proposal = StewardProposal::new(
            ProposalId::new(),
            StewardIdentity::new("deterministic-mock", "0.1.0", "strict")?,
            StewardAction::CreateCellDraft {
                anchors: vec![SemanticAnchor::new("project:continuitydb:status")],
                payload_text: "ContinuityDB has a Steward proposal substrate.".to_string(),
            },
            "Evidence supports creating a draft cell.",
            vec!["test://evidence".to_string()],
            created_at(),
        )?;

        assert!(matches!(
            proposal.action(),
            StewardAction::CreateCellDraft { payload_text, .. }
                if payload_text == "ContinuityDB has a Steward proposal substrate."
        ));
        Ok(())
    }
}
```

- [ ] **Step 2: Run tests to verify failure**

Run:

```bash
cargo test -p continuitydb-steward
```

Expected: FAIL because `error.rs` and `proposal.rs` do not exist.

- [ ] **Step 3: Add error type**

Create `crates/continuitydb-steward/src/error.rs`:

```rust
//! Error types for Steward proposal handling.

use thiserror::Error;

/// Errors produced by Steward proposal construction or ledger recording.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum StewardError {
    /// Steward identity fields must be non-empty.
    #[error("steward identity fields must be non-empty")]
    EmptyStewardIdentity,
    /// Proposal rationale must be non-empty.
    #[error("proposal rationale must be non-empty")]
    EmptyRationale,
    /// Proposal citations must be non-empty.
    #[error("proposal must include at least one citation")]
    MissingCitations,
    /// Proposal decision IDs must match the recorded proposal.
    #[error("proposal decision ID does not match proposal ID")]
    MismatchedDecision,
}
```

- [ ] **Step 4: Add proposal types**

Create `crates/continuitydb-steward/src/proposal.rs`:

```rust
//! Steward proposal types.

use chrono::{DateTime, Utc};
use continuitydb_core::{SemanticAnchor, StateCellId};
use continuitydb_revision::RevisionLinkKind;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::StewardError;

/// Immutable identifier for a Steward proposal.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct ProposalId(Uuid);

impl ProposalId {
    /// Creates a random proposal identifier.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for ProposalId {
    fn default() -> Self {
        Self::new()
    }
}

/// Identifies the steward implementation or model that emitted a proposal.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StewardIdentity {
    name: String,
    version: String,
    prompt_profile: String,
}

impl StewardIdentity {
    /// Creates a validated Steward identity.
    pub fn new(
        name: impl Into<String>,
        version: impl Into<String>,
        prompt_profile: impl Into<String>,
    ) -> Result<Self, StewardError> {
        let name = name.into().trim().to_string();
        let version = version.into().trim().to_string();
        let prompt_profile = prompt_profile.into().trim().to_string();

        if name.is_empty() || version.is_empty() || prompt_profile.is_empty() {
            return Err(StewardError::EmptyStewardIdentity);
        }

        Ok(Self {
            name,
            version,
            prompt_profile,
        })
    }
}

/// Intent proposed by a database Steward.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum StewardAction {
    /// Proposes a new StateCell draft without committing it.
    CreateCellDraft {
        /// Semantic anchors for the proposed cell.
        anchors: Vec<SemanticAnchor>,
        /// Draft text payload.
        payload_text: String,
    },
    /// Proposes a revision link between two StateCell versions.
    LinkRevision {
        /// Source StateCell version.
        source: StateCellId,
        /// Revision link kind.
        kind: RevisionLinkKind,
        /// Target StateCell version.
        target: StateCellId,
    },
    /// Proposes a confidence value for an existing StateCell.
    AdjustConfidence {
        /// Target StateCell version.
        cell_id: StateCellId,
        /// Proposed confidence in the inclusive range 0.0..=1.0.
        proposed_confidence: f32,
    },
    /// Proposes answerability questions for an existing StateCell.
    LabelAnswerability {
        /// Target StateCell version.
        cell_id: StateCellId,
        /// Proposed questions.
        questions: Vec<String>,
    },
    /// Proposes that a StateCell should enter frontier monitoring.
    MarkFrontier {
        /// Target StateCell version.
        cell_id: StateCellId,
    },
    /// Proposes verification work.
    RequestVerification {
        /// Target StateCell version when the request is cell-specific.
        cell_id: Option<StateCellId>,
        /// Verification request description.
        request: String,
    },
}

/// Structured proposal emitted by a deterministic or model-backed Steward.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StewardProposal {
    id: ProposalId,
    steward: StewardIdentity,
    action: StewardAction,
    rationale: String,
    citations: Vec<String>,
    created_at: DateTime<Utc>,
}

impl StewardProposal {
    /// Creates a validated Steward proposal.
    pub fn new(
        id: ProposalId,
        steward: StewardIdentity,
        action: StewardAction,
        rationale: impl Into<String>,
        citations: Vec<String>,
        created_at: DateTime<Utc>,
    ) -> Result<Self, StewardError> {
        let rationale = rationale.into().trim().to_string();
        let citations: Vec<String> = citations
            .into_iter()
            .map(|citation| citation.trim().to_string())
            .filter(|citation| !citation.is_empty())
            .collect();

        if rationale.is_empty() {
            return Err(StewardError::EmptyRationale);
        }

        if citations.is_empty() {
            return Err(StewardError::MissingCitations);
        }

        Ok(Self {
            id,
            steward,
            action,
            rationale,
            citations,
            created_at,
        })
    }

    /// Returns this proposal's identifier.
    pub fn id(&self) -> ProposalId {
        self.id
    }

    /// Returns the proposing Steward identity.
    pub fn steward(&self) -> &StewardIdentity {
        &self.steward
    }

    /// Returns the proposed action.
    pub fn action(&self) -> &StewardAction {
        &self.action
    }

    /// Returns the proposal rationale.
    pub fn rationale(&self) -> &str {
        &self.rationale
    }

    /// Returns supporting citation locators.
    pub fn citations(&self) -> &[String] {
        &self.citations
    }

    /// Returns creation time.
    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }
}
```

- [ ] **Step 5: Run proposal tests**

Run:

```bash
cargo test -p continuitydb-steward
```

Expected: PASS for proposal construction tests.

- [ ] **Step 6: Commit proposal types**

Run:

```bash
git add Cargo.toml Cargo.lock crates/continuitydb-steward
git commit -m "feat: add steward proposal types"
```

## Task 2: Strict Proposal Policy

**Files:**
- Modify: `crates/continuitydb-steward/src/lib.rs`
- Create: `crates/continuitydb-steward/src/policy.rs`
- Modify: `crates/continuitydb-steward/src/proposal.rs`

- [ ] **Step 1: Write failing policy tests**

Modify `crates/continuitydb-steward/src/lib.rs`:

```rust
//! Deterministic Steward proposal substrate.

mod error;
mod policy;
mod proposal;

pub use error::StewardError;
pub use policy::{ProposalDecision, ProposalOutcome, ProposalPolicy};
pub use proposal::{ProposalId, StewardAction, StewardIdentity, StewardProposal};

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use continuitydb_core::{SemanticAnchor, StateCellId};
    use continuitydb_revision::RevisionLinkKind;

    use super::{
        ProposalId, ProposalOutcome, ProposalPolicy, StewardAction, StewardError, StewardIdentity,
        StewardProposal,
    };

    fn created_at() -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 5, 20, 0, 0, 0)
            .single()
            .unwrap_or_else(Utc::now)
    }

    fn steward() -> Result<StewardIdentity, StewardError> {
        StewardIdentity::new("deterministic-mock", "0.1.0", "strict")
    }

    fn valid_link_revision() -> Result<StewardProposal, StewardError> {
        StewardProposal::new(
            ProposalId::new(),
            steward()?,
            StewardAction::LinkRevision {
                source: StateCellId::new(),
                kind: RevisionLinkKind::Supersedes,
                target: StateCellId::new(),
            },
            "New evidence supersedes the prior cell.",
            vec!["test://evidence".to_string()],
            created_at(),
        )
    }

    #[test]
    fn strict_policy_accepts_structurally_valid_link_revision() -> Result<(), Box<dyn std::error::Error>> {
        let proposal = valid_link_revision()?;
        let decision = ProposalPolicy::strict().evaluate(&proposal, created_at());

        assert_eq!(decision.proposal_id(), proposal.id());
        assert_eq!(decision.outcome(), ProposalOutcome::Accepted);
        assert_eq!(decision.reasons(), &["policy:structurally-valid".to_string()]);
        Ok(())
    }

    #[test]
    fn strict_policy_rejects_empty_answerability_questions() -> Result<(), Box<dyn std::error::Error>> {
        let proposal = StewardProposal::new(
            ProposalId::new(),
            steward()?,
            StewardAction::LabelAnswerability {
                cell_id: StateCellId::new(),
                questions: vec![" ".to_string()],
            },
            "The cell should answer a task question.",
            vec!["test://evidence".to_string()],
            created_at(),
        )?;

        let decision = ProposalPolicy::strict().evaluate(&proposal, created_at());

        assert_eq!(decision.outcome(), ProposalOutcome::Rejected);
        assert_eq!(decision.reasons(), &["policy:empty-answerability".to_string()]);
        Ok(())
    }

    #[test]
    fn strict_policy_rejects_empty_create_cell_draft() -> Result<(), Box<dyn std::error::Error>> {
        let proposal = StewardProposal::new(
            ProposalId::new(),
            steward()?,
            StewardAction::CreateCellDraft {
                anchors: Vec::new(),
                payload_text: " ".to_string(),
            },
            "Evidence suggests a new cell.",
            vec!["test://evidence".to_string()],
            created_at(),
        )?;

        let decision = ProposalPolicy::strict().evaluate(&proposal, created_at());

        assert_eq!(decision.outcome(), ProposalOutcome::Rejected);
        assert_eq!(
            decision.reasons(),
            &[
                "policy:missing-create-cell-anchor".to_string(),
                "policy:empty-create-cell-payload".to_string(),
            ]
        );
        Ok(())
    }

    #[test]
    fn strict_policy_rejects_invalid_confidence_adjustment() -> Result<(), Box<dyn std::error::Error>> {
        let proposal = StewardProposal::new(
            ProposalId::new(),
            steward()?,
            StewardAction::AdjustConfidence {
                cell_id: StateCellId::new(),
                proposed_confidence: 1.2,
            },
            "The proposed confidence is out of range.",
            vec!["test://evidence".to_string()],
            created_at(),
        )?;

        let decision = ProposalPolicy::strict().evaluate(&proposal, created_at());

        assert_eq!(decision.outcome(), ProposalOutcome::Rejected);
        assert_eq!(decision.reasons(), &["policy:invalid-confidence".to_string()]);
        Ok(())
    }

    #[test]
    fn proposal_carries_confidence_and_answerability_intent() -> Result<(), Box<dyn std::error::Error>> {
        let cell_id = StateCellId::new();
        let confidence = StewardAction::AdjustConfidence {
            cell_id,
            proposed_confidence: 0.8,
        };
        let answerability = StewardAction::LabelAnswerability {
            cell_id,
            questions: vec!["what changed?".to_string()],
        };

        assert!(matches!(
            confidence,
            StewardAction::AdjustConfidence {
                proposed_confidence: actual,
                ..
            } if actual == 0.8
        ));
        assert!(matches!(
            answerability,
            StewardAction::LabelAnswerability { questions, .. } if questions == vec!["what changed?".to_string()]
        ));
        Ok(())
    }
}
```

Keep the existing proposal tests that are not repeated here when editing the file. The important new failing tests are the three policy tests.

- [ ] **Step 2: Run tests to verify failure**

Run:

```bash
cargo test -p continuitydb-steward
```

Expected: FAIL because `policy.rs`, `ProposalPolicy`, `ProposalDecision`, and `ProposalOutcome` do not exist.

- [ ] **Step 3: Add action validation helpers**

Add this method to `impl StewardAction` in `crates/continuitydb-steward/src/proposal.rs`:

```rust
impl StewardAction {
    /// Returns deterministic policy rejection reasons for invalid action payloads.
    pub fn validation_reasons(&self) -> Vec<String> {
        match self {
            Self::CreateCellDraft {
                anchors,
                payload_text,
            } => {
                let mut reasons = Vec::new();
                if anchors.is_empty() {
                    reasons.push("policy:missing-create-cell-anchor".to_string());
                }
                if payload_text.trim().is_empty() {
                    reasons.push("policy:empty-create-cell-payload".to_string());
                }
                reasons
            }
            Self::LabelAnswerability { questions, .. } => {
                if questions.iter().any(|question| !question.trim().is_empty()) {
                    Vec::new()
                } else {
                    vec!["policy:empty-answerability".to_string()]
                }
            }
            Self::AdjustConfidence {
                proposed_confidence, ..
            } => {
                if (0.0..=1.0).contains(proposed_confidence) {
                    Vec::new()
                } else {
                    vec!["policy:invalid-confidence".to_string()]
                }
            }
            Self::RequestVerification { request, .. } => {
                if request.trim().is_empty() {
                    vec!["policy:empty-verification-request".to_string()]
                } else {
                    Vec::new()
                }
            }
            Self::LinkRevision { .. } | Self::MarkFrontier { .. } => {
                Vec::new()
            }
        }
    }
}
```

- [ ] **Step 4: Add policy types**

Create `crates/continuitydb-steward/src/policy.rs`:

```rust
//! Deterministic proposal policy.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{ProposalId, StewardProposal};

/// Policy outcome for a Steward proposal.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ProposalOutcome {
    /// Proposal passed deterministic policy.
    Accepted,
    /// Proposal failed deterministic policy.
    Rejected,
}

/// Deterministic decision produced by proposal policy evaluation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProposalDecision {
    proposal_id: ProposalId,
    outcome: ProposalOutcome,
    reasons: Vec<String>,
    decided_at: DateTime<Utc>,
}

impl ProposalDecision {
    /// Creates a decision.
    pub fn new(
        proposal_id: ProposalId,
        outcome: ProposalOutcome,
        reasons: Vec<String>,
        decided_at: DateTime<Utc>,
    ) -> Self {
        Self {
            proposal_id,
            outcome,
            reasons,
            decided_at,
        }
    }

    /// Returns the proposal ID this decision applies to.
    pub fn proposal_id(&self) -> ProposalId {
        self.proposal_id
    }

    /// Returns the decision outcome.
    pub fn outcome(&self) -> ProposalOutcome {
        self.outcome
    }

    /// Returns deterministic decision reasons.
    pub fn reasons(&self) -> &[String] {
        &self.reasons
    }

    /// Returns decision time.
    pub fn decided_at(&self) -> DateTime<Utc> {
        self.decided_at
    }
}

/// Deterministic Steward proposal policy.
#[derive(Clone, Debug, Default)]
pub struct ProposalPolicy;

impl ProposalPolicy {
    /// Creates the strict built-in proposal policy.
    pub fn strict() -> Self {
        Self
    }

    /// Evaluates a proposal without mutating database state.
    pub fn evaluate(
        &self,
        proposal: &StewardProposal,
        decided_at: DateTime<Utc>,
    ) -> ProposalDecision {
        let mut reasons = proposal.action().validation_reasons();
        if reasons.is_empty() {
            reasons.push("policy:structurally-valid".to_string());
            ProposalDecision::new(
                proposal.id(),
                ProposalOutcome::Accepted,
                reasons,
                decided_at,
            )
        } else {
            ProposalDecision::new(
                proposal.id(),
                ProposalOutcome::Rejected,
                reasons,
                decided_at,
            )
        }
    }
}
```

- [ ] **Step 5: Run policy tests**

Run:

```bash
cargo test -p continuitydb-steward
```

Expected: PASS for proposal and policy tests.

- [ ] **Step 6: Commit strict policy**

Run:

```bash
git add crates/continuitydb-steward
git commit -m "feat: add steward proposal policy"
```

## Task 3: Proposal Ledger

**Files:**
- Modify: `crates/continuitydb-steward/src/lib.rs`
- Create: `crates/continuitydb-steward/src/ledger.rs`
- Modify: `crates/continuitydb-steward/src/error.rs`

- [ ] **Step 1: Write failing ledger tests**

Add `mod ledger;` and export ledger types in `crates/continuitydb-steward/src/lib.rs`:

```rust
mod ledger;
pub use ledger::{ProposalAuditRecord, ProposalLedger};
```

Add these tests to the existing test module:

```rust
#[test]
fn ledger_preserves_accepted_and_rejected_proposals() -> Result<(), Box<dyn std::error::Error>> {
    let decided_at = created_at();
    let accepted = valid_link_revision()?;
    let rejected = StewardProposal::new(
        ProposalId::new(),
        steward()?,
        StewardAction::LabelAnswerability {
            cell_id: StateCellId::new(),
            questions: vec![" ".to_string()],
        },
        "The cell should answer a task question.",
        vec!["test://evidence".to_string()],
        created_at(),
    )?;
    let policy = ProposalPolicy::strict();
    let accepted_decision = policy.evaluate(&accepted, decided_at);
    let rejected_decision = policy.evaluate(&rejected, decided_at);
    let mut ledger = ProposalLedger::default();

    ledger.record(accepted.clone(), accepted_decision)?;
    ledger.record(rejected.clone(), rejected_decision)?;

    let records = ledger.records();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].proposal().id(), accepted.id());
    assert_eq!(records[0].decision().outcome(), ProposalOutcome::Accepted);
    assert_eq!(records[1].proposal().id(), rejected.id());
    assert_eq!(records[1].decision().outcome(), ProposalOutcome::Rejected);
    assert_eq!(
        ledger.record_by_id(rejected.id()).map(|record| record.proposal().id()),
        Some(rejected.id())
    );
    Ok(())
}

#[test]
fn ledger_rejects_mismatched_proposal_and_decision_ids() -> Result<(), Box<dyn std::error::Error>> {
    let proposal = valid_link_revision()?;
    let mismatched_decision = ProposalDecision::new(
        ProposalId::new(),
        ProposalOutcome::Accepted,
        vec!["policy:structurally-valid".to_string()],
        created_at(),
    );
    let mut ledger = ProposalLedger::default();

    let result = ledger.record(proposal, mismatched_decision);

    assert!(matches!(result, Err(StewardError::MismatchedDecision)));
    Ok(())
}
```

- [ ] **Step 2: Run tests to verify failure**

Run:

```bash
cargo test -p continuitydb-steward
```

Expected: FAIL because `ledger.rs`, `ProposalAuditRecord`, and `ProposalLedger` do not exist.

- [ ] **Step 3: Add ledger implementation**

Create `crates/continuitydb-steward/src/ledger.rs`:

```rust
//! Append-only proposal audit ledger.

use crate::{ProposalDecision, ProposalId, StewardError, StewardProposal};

/// Audit record preserving a proposal and its deterministic policy decision.
#[derive(Clone, Debug, PartialEq)]
pub struct ProposalAuditRecord {
    proposal: StewardProposal,
    decision: ProposalDecision,
}

impl ProposalAuditRecord {
    /// Creates an audit record after checking proposal and decision IDs match.
    pub fn new(
        proposal: StewardProposal,
        decision: ProposalDecision,
    ) -> Result<Self, StewardError> {
        if proposal.id() != decision.proposal_id() {
            return Err(StewardError::MismatchedDecision);
        }

        Ok(Self { proposal, decision })
    }

    /// Returns the recorded proposal.
    pub fn proposal(&self) -> &StewardProposal {
        &self.proposal
    }

    /// Returns the recorded decision.
    pub fn decision(&self) -> &ProposalDecision {
        &self.decision
    }
}

/// In-memory append-only proposal audit ledger.
#[derive(Default)]
pub struct ProposalLedger {
    records: Vec<ProposalAuditRecord>,
}

impl ProposalLedger {
    /// Records a proposal and decision pair.
    pub fn record(
        &mut self,
        proposal: StewardProposal,
        decision: ProposalDecision,
    ) -> Result<(), StewardError> {
        self.records
            .push(ProposalAuditRecord::new(proposal, decision)?);
        Ok(())
    }

    /// Returns all audit records in insertion order.
    pub fn records(&self) -> &[ProposalAuditRecord] {
        &self.records
    }

    /// Returns an audit record by proposal ID.
    pub fn record_by_id(&self, proposal_id: ProposalId) -> Option<&ProposalAuditRecord> {
        self.records
            .iter()
            .find(|record| record.proposal().id() == proposal_id)
    }
}
```

- [ ] **Step 4: Run ledger tests**

Run:

```bash
cargo test -p continuitydb-steward
```

Expected: PASS for all Steward tests.

- [ ] **Step 5: Commit ledger**

Run:

```bash
git add crates/continuitydb-steward
git commit -m "feat: add steward proposal ledger"
```

## Task 4: Workspace Verification And Roadmap Update

**Files:**
- Modify: `README.md`
- Modify: `docs/roadmap.md`

- [ ] **Step 1: Update README crate list**

Modify `README.md` current scope list to include:

```markdown
- Deterministic Steward proposal substrate.
```

- [ ] **Step 2: Update roadmap milestone status**

Modify `docs/roadmap.md` under `## Steward Milestones`:

```markdown
1. Define `StewardProposal` types without invoking any model. Implemented in `continuitydb-steward`.
2. Add policy validation for accepting and rejecting proposals. Implemented in `continuitydb-steward`.
3. Persist accepted and rejected proposals for audit. Implemented as an in-memory append-only ledger in `continuitydb-steward`; storage-backed persistence remains future work.
4. Build a deterministic mock steward for test-first development.
5. Add local model inference behind a feature flag.
6. Evaluate small open-source steward models against fixed proposal-quality tests.
7. Add frontier/watch integration so the Steward can propose refresh and verification work.
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
git add README.md docs/roadmap.md Cargo.toml Cargo.lock crates/continuitydb-steward
git commit -m "docs: mark steward substrate roadmap progress"
```

If there are no doc or verification fixes after Steps 1-5, do not create an empty commit.

## Self-Review Against Spec

- `continuitydb-steward` crate: Task 1.
- No model inference dependencies: Task 1 manifest and Task 4 clippy/test verification.
- Proposal types: Task 1.
- Policy decisions: Task 2.
- Accepted and rejected audit ledger: Task 3.
- Missing citations rejected: Task 1 constructor test.
- Empty rationale rejected: Task 1 constructor test.
- Empty steward identity rejected: Task 1 constructor test.
- Invalid confidence adjustments rejected: Task 2 policy test.
- Empty answerability labels rejected: Task 2 policy test.
- Structurally valid `LinkRevision` accepted: Task 2 policy test.
- Ledger mismatched proposal/decision IDs rejected: Task 3 ledger test.
- Ledger preserves insertion order: Task 3 ledger test.
- Workspace verification: Task 4.

## Execution Notes

This plan intentionally avoids Qwen, llama.cpp, mistral.rs, model downloads, async workers, storage persistence, and applying accepted proposals to core database state. Those are later frontier roadmap slices.
