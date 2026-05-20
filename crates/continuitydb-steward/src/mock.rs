//! Deterministic mock Steward.

use chrono::{DateTime, Utc};
use continuitydb_core::{SemanticAnchor, StateCellId};
use continuitydb_revision::RevisionLinkKind;

use crate::{ProposalId, StewardAction, StewardError, StewardIdentity, StewardProposal};

/// Deterministic Steward used for tests and local development.
#[derive(Clone, Debug)]
pub struct MockSteward {
    identity: StewardIdentity,
}

impl MockSteward {
    /// Creates a mock Steward with a stable identity.
    pub fn new(identity: StewardIdentity) -> Self {
        Self { identity }
    }

    /// Converts deterministic input rules into validated proposals.
    pub fn propose(&self, input: MockStewardInput) -> Result<Vec<StewardProposal>, StewardError> {
        input
            .rules
            .into_iter()
            .map(|rule| rule.into_proposal(self.identity.clone(), input.created_at))
            .collect()
    }
}

/// Deterministic proposal input for [`MockSteward`].
#[derive(Clone, Debug)]
pub struct MockStewardInput {
    created_at: DateTime<Utc>,
    rules: Vec<MockStewardRule>,
}

impl MockStewardInput {
    /// Creates empty mock input with a shared proposal creation timestamp.
    pub fn new(created_at: DateTime<Utc>) -> Self {
        Self {
            created_at,
            rules: Vec::new(),
        }
    }

    /// Appends a deterministic proposal rule.
    pub fn with_rule(mut self, rule: MockStewardRule) -> Self {
        self.rules.push(rule);
        self
    }
}

/// Deterministic rules that emit one Steward proposal each.
#[derive(Clone, Debug, PartialEq)]
pub enum MockStewardRule {
    /// Emits a cell draft proposal.
    CreateCellDraft {
        /// Semantic anchors for the proposed cell.
        anchors: Vec<SemanticAnchor>,
        /// Draft text payload.
        payload_text: String,
        /// Human-readable proposal rationale.
        rationale: String,
        /// Supporting citation locators.
        citations: Vec<String>,
    },
    /// Emits a revision link proposal.
    LinkRevision {
        /// Source StateCell version.
        source: StateCellId,
        /// Revision link kind.
        kind: RevisionLinkKind,
        /// Target StateCell version.
        target: StateCellId,
        /// Human-readable proposal rationale.
        rationale: String,
        /// Supporting citation locators.
        citations: Vec<String>,
    },
    /// Emits a confidence adjustment proposal.
    AdjustConfidence {
        /// Target StateCell version.
        cell_id: StateCellId,
        /// Proposed confidence in the inclusive range 0.0..=1.0.
        proposed_confidence: f32,
        /// Human-readable proposal rationale.
        rationale: String,
        /// Supporting citation locators.
        citations: Vec<String>,
    },
    /// Emits an answerability labeling proposal.
    LabelAnswerability {
        /// Target StateCell version.
        cell_id: StateCellId,
        /// Proposed answerability questions.
        questions: Vec<String>,
        /// Human-readable proposal rationale.
        rationale: String,
        /// Supporting citation locators.
        citations: Vec<String>,
    },
    /// Emits a frontier monitoring proposal.
    MarkFrontier {
        /// Target StateCell version.
        cell_id: StateCellId,
        /// Human-readable proposal rationale.
        rationale: String,
        /// Supporting citation locators.
        citations: Vec<String>,
    },
    /// Emits a verification request proposal.
    RequestVerification {
        /// Target StateCell version when the request is cell-specific.
        cell_id: Option<StateCellId>,
        /// Verification request description.
        request: String,
        /// Human-readable proposal rationale.
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
            } => (
                StewardAction::MarkFrontier { cell_id },
                rationale,
                citations,
            ),
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
            ProposalId::new(),
            steward,
            action,
            rationale,
            citations,
            created_at,
        )
    }
}
