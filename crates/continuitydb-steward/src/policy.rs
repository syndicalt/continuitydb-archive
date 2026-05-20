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
