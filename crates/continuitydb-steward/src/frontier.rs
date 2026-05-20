//! Deterministic frontier watch Steward integration.

use chrono::{DateTime, Utc};
use continuitydb_core::StateCellId;

use crate::{ProposalId, StewardAction, StewardError, StewardIdentity, StewardProposal};

/// Signal observed by frontier/watch integration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrontierWatchSignal {
    /// Evidence for a watched cell is stale and needs refresh.
    StaleEvidence,
    /// A cell is uncertain enough and important enough to enter frontier monitoring.
    HighImpactUncertainty,
    /// The watch event does not require Steward work.
    Benign,
}

/// Evidence-backed frontier watch event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrontierWatchEvent {
    cell_id: StateCellId,
    signal: FrontierWatchSignal,
    citation: String,
    observed_at: DateTime<Utc>,
}

impl FrontierWatchEvent {
    /// Creates a frontier watch event.
    pub fn new(
        cell_id: StateCellId,
        signal: FrontierWatchSignal,
        citation: impl Into<String>,
        observed_at: DateTime<Utc>,
    ) -> Self {
        Self {
            cell_id,
            signal,
            citation: citation.into(),
            observed_at,
        }
    }

    /// Returns the watched cell ID.
    pub fn cell_id(&self) -> StateCellId {
        self.cell_id
    }

    /// Returns the frontier watch signal.
    pub fn signal(&self) -> FrontierWatchSignal {
        self.signal
    }

    /// Returns the event citation locator.
    pub fn citation(&self) -> &str {
        &self.citation
    }

    /// Returns when the event was observed.
    pub fn observed_at(&self) -> DateTime<Utc> {
        self.observed_at
    }
}

/// Deterministic Steward for frontier/watch events.
#[derive(Clone, Debug)]
pub struct FrontierSteward {
    identity: StewardIdentity,
}

impl FrontierSteward {
    /// Creates a frontier Steward with a stable identity.
    pub fn new(identity: StewardIdentity) -> Self {
        Self { identity }
    }

    /// Converts frontier watch events into validated Steward proposals.
    pub fn propose(
        &self,
        events: Vec<FrontierWatchEvent>,
        created_at: DateTime<Utc>,
    ) -> Result<Vec<StewardProposal>, StewardError> {
        let mut proposals = Vec::new();
        for event in events {
            if let Some(proposal) = self.proposal_for_event(event, created_at) {
                proposals.push(proposal?);
            }
        }
        Ok(proposals)
    }

    fn proposal_for_event(
        &self,
        event: FrontierWatchEvent,
        created_at: DateTime<Utc>,
    ) -> Option<Result<StewardProposal, StewardError>> {
        match event.signal {
            FrontierWatchSignal::StaleEvidence => Some(StewardProposal::new(
                ProposalId::new(),
                self.identity.clone(),
                StewardAction::RequestVerification {
                    cell_id: Some(event.cell_id),
                    request: "Refresh stale evidence for frontier cell.".to_string(),
                },
                "Watched frontier evidence is stale and requires verification.",
                vec![event.citation],
                created_at,
            )),
            FrontierWatchSignal::HighImpactUncertainty => Some(StewardProposal::new(
                ProposalId::new(),
                self.identity.clone(),
                StewardAction::MarkFrontier {
                    cell_id: event.cell_id,
                },
                "Watched cell has high-impact uncertainty.",
                vec![event.citation],
                created_at,
            )),
            FrontierWatchSignal::Benign => None,
        }
    }
}
