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

/// Storage contract for append-only proposal audit records.
pub trait ProposalLedgerStore {
    /// Appends an audit record.
    fn append_record(&mut self, record: ProposalAuditRecord) -> Result<(), StewardError>;

    /// Lists all audit records in insertion order.
    fn list_records(&self) -> Result<Vec<ProposalAuditRecord>, StewardError>;

    /// Returns an audit record by proposal ID.
    fn get_record(
        &self,
        proposal_id: ProposalId,
    ) -> Result<Option<ProposalAuditRecord>, StewardError>;
}

/// In-memory proposal ledger store implementation for correctness tests.
#[derive(Default)]
pub struct MemoryProposalStore {
    records: Vec<ProposalAuditRecord>,
}

impl ProposalLedgerStore for MemoryProposalStore {
    fn append_record(&mut self, record: ProposalAuditRecord) -> Result<(), StewardError> {
        self.records.push(record);
        Ok(())
    }

    fn list_records(&self) -> Result<Vec<ProposalAuditRecord>, StewardError> {
        Ok(self.records.clone())
    }

    fn get_record(
        &self,
        proposal_id: ProposalId,
    ) -> Result<Option<ProposalAuditRecord>, StewardError> {
        Ok(self
            .records
            .iter()
            .find(|record| record.proposal().id() == proposal_id)
            .cloned())
    }
}

/// Proposal ledger backed by a pluggable storage implementation.
pub struct StoredProposalLedger<S> {
    store: S,
}

impl<S> StoredProposalLedger<S>
where
    S: ProposalLedgerStore,
{
    /// Creates a stored proposal ledger.
    pub fn new(store: S) -> Self {
        Self { store }
    }

    /// Records a proposal and decision pair after validation.
    pub fn record(
        &mut self,
        proposal: StewardProposal,
        decision: ProposalDecision,
    ) -> Result<(), StewardError> {
        let record = ProposalAuditRecord::new(proposal, decision)?;
        self.store.append_record(record)
    }

    /// Returns all audit records in insertion order.
    pub fn records(&self) -> Result<Vec<ProposalAuditRecord>, StewardError> {
        self.store.list_records()
    }

    /// Returns an audit record by proposal ID.
    pub fn record_by_id(
        &self,
        proposal_id: ProposalId,
    ) -> Result<Option<ProposalAuditRecord>, StewardError> {
        self.store.get_record(proposal_id)
    }

    /// Returns the backing store.
    pub fn into_store(self) -> S {
        self.store
    }
}
