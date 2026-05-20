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
