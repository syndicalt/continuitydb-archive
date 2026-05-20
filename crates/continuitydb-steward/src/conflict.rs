//! Deterministic conflict-resolution Steward integration.

use chrono::{DateTime, Utc};
use continuitydb_kernel::StorageKernel;
use continuitydb_revision::{
    ConflictResolutionRecommendation, ConflictResolutionScan, RevisionLinkKind,
};

use crate::{
    KernelProposalStore, ProposalId, ProposalLedgerStore, ProposalPolicy, StewardAction,
    StewardError, StewardIdentity, StewardProposal, StoredProposalLedger,
};

/// Deterministic Steward for revision conflict-resolution recommendations.
#[derive(Clone, Debug)]
pub struct ConflictResolutionSteward {
    identity: StewardIdentity,
}

impl ConflictResolutionSteward {
    /// Creates a conflict-resolution Steward with a stable identity.
    pub fn new(identity: StewardIdentity) -> Self {
        Self { identity }
    }

    /// Converts deterministic conflict-resolution recommendations into Steward proposals.
    pub fn propose(
        &self,
        scan: ConflictResolutionScan,
        created_at: DateTime<Utc>,
    ) -> Result<Vec<StewardProposal>, StewardError> {
        scan.recommendations
            .into_iter()
            .map(|recommendation| self.proposal_for_recommendation(recommendation, created_at))
            .collect()
    }

    /// Converts recommendations into proposals, evaluates policy, and records audit entries.
    pub fn propose_and_record<S>(
        &self,
        scan: ConflictResolutionScan,
        created_at: DateTime<Utc>,
        policy: &ProposalPolicy,
        ledger: &mut StoredProposalLedger<S>,
    ) -> Result<Vec<StewardProposal>, StewardError>
    where
        S: ProposalLedgerStore,
    {
        let proposals = self.propose(scan, created_at)?;
        for proposal in proposals.iter().cloned() {
            let decision = policy.evaluate(&proposal, created_at);
            ledger.record(proposal, decision)?;
        }
        Ok(proposals)
    }

    /// Converts recommendations into proposals and records audit entries as StateCells.
    pub fn propose_and_record_to_kernel<K>(
        &self,
        scan: ConflictResolutionScan,
        created_at: DateTime<Utc>,
        policy: &ProposalPolicy,
        kernel: K,
    ) -> Result<(Vec<StewardProposal>, K), StewardError>
    where
        K: StorageKernel,
    {
        let mut ledger = StoredProposalLedger::new(KernelProposalStore::new(kernel));
        let proposals = self.propose_and_record(scan, created_at, policy, &mut ledger)?;
        let store = ledger.into_store();
        Ok((proposals, store.into_kernel()))
    }

    fn proposal_for_recommendation(
        &self,
        recommendation: ConflictResolutionRecommendation,
        created_at: DateTime<Utc>,
    ) -> Result<StewardProposal, StewardError> {
        let citation = format!("policy://continuitydb/revision/{}", recommendation.reason);
        let action = match (recommendation.winner, recommendation.loser) {
            (Some(winner), Some(loser)) => StewardAction::LinkRevision {
                source: winner,
                kind: RevisionLinkKind::Supersedes,
                target: loser,
            },
            _ => StewardAction::RequestVerification {
                cell_id: Some(recommendation.conflict.left),
                request: "Review unresolved StateCell conflict before accepting a revision."
                    .to_string(),
            },
        };

        StewardProposal::new(
            ProposalId::new(),
            self.identity.clone(),
            action,
            format!(
                "Deterministic conflict policy recommended {:?}.",
                recommendation.kind
            ),
            vec![citation],
            created_at,
        )
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use continuitydb_core::{
        Answerability, CellCost, CellPayload, Citation, Confidence, Evidence, Scope,
        SemanticAnchor, SourceId, StateCell, StateCellId, TrustSignal, ValidTimeRange,
    };
    use continuitydb_kernel::{CellLookup, StorageKernel};
    use continuitydb_revision::{recommend_conflict_resolutions, RevisionLinkKind};

    use crate::{
        ConflictResolutionSteward, MemoryProposalStore, ProposalOutcome, ProposalPolicy,
        StewardAction, StewardError, StewardIdentity, StoredProposalLedger,
    };

    fn created_at() -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 5, 20, 0, 0, 0)
            .single()
            .unwrap_or_else(Utc::now)
    }

    fn timestamp(day: u32) -> Result<chrono::DateTime<Utc>, Box<dyn std::error::Error>> {
        Utc.with_ymd_and_hms(2026, 5, day, 0, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp").into())
    }

    fn steward() -> Result<StewardIdentity, StewardError> {
        StewardIdentity::new(
            "conflict-resolution-steward",
            "0.1.0",
            "deterministic-policy",
        )
    }

    fn sample_cell(
        payload: &str,
        from_day: u32,
        confidence: f32,
    ) -> Result<StateCell, Box<dyn std::error::Error>> {
        StateCell::new(
            StateCellId::new(),
            vec![SemanticAnchor::new("project:continuitydb:release-status")],
            ValidTimeRange::new(timestamp(from_day)?, Some(timestamp(23)?))?,
            Scope::Project("continuitydb".to_string()),
            Answerability::new(vec!["what is release status?".to_string()])?,
            vec![Evidence {
                source: SourceId::new("test"),
                citation: Citation {
                    locator: "test://release-status".to_string(),
                },
                confidence: Confidence::new(confidence)?,
                trust: vec![TrustSignal::DirectObservation],
            }],
            CellPayload::Text(payload.to_string()),
            CellCost::new(5, 0)?,
        )
        .map_err(Into::into)
    }

    #[test]
    fn conflict_resolution_steward_emits_supersession_link_proposal(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let low_confidence = sample_cell("Release is blocked.", 20, 0.55)?;
        let high_confidence = sample_cell("Release is green.", 21, 0.9)?;
        let scan =
            recommend_conflict_resolutions(&[low_confidence.clone(), high_confidence.clone()]);
        let steward = ConflictResolutionSteward::new(steward()?);

        let proposals = steward.propose(scan, created_at())?;

        assert_eq!(proposals.len(), 1);
        assert_eq!(
            proposals[0].action(),
            &StewardAction::LinkRevision {
                source: high_confidence.id,
                kind: RevisionLinkKind::Supersedes,
                target: low_confidence.id,
            }
        );
        assert_eq!(
            proposals[0].citations(),
            &["policy://continuitydb/revision/resolution:confidence-gap".to_string()]
        );
        let decision = ProposalPolicy::strict().evaluate(&proposals[0], created_at());
        assert_eq!(decision.outcome(), ProposalOutcome::Accepted);
        Ok(())
    }

    #[test]
    fn conflict_resolution_steward_records_policy_decisions(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let low_confidence = sample_cell("Release is blocked.", 20, 0.55)?;
        let high_confidence = sample_cell("Release is green.", 21, 0.9)?;
        let scan =
            recommend_conflict_resolutions(&[low_confidence.clone(), high_confidence.clone()]);
        let steward = ConflictResolutionSteward::new(steward()?);
        let mut ledger = StoredProposalLedger::new(MemoryProposalStore::default());

        let proposals = steward.propose_and_record(
            scan,
            created_at(),
            &ProposalPolicy::strict(),
            &mut ledger,
        )?;

        assert_eq!(proposals.len(), 1);
        let records = ledger.records()?;
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].proposal().id(), proposals[0].id());
        assert_eq!(records[0].decision().outcome(), ProposalOutcome::Accepted);
        Ok(())
    }

    #[test]
    fn conflict_resolution_steward_records_to_kernel_cells(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let low_confidence = sample_cell("Release is blocked.", 20, 0.55)?;
        let high_confidence = sample_cell("Release is green.", 21, 0.9)?;
        let scan =
            recommend_conflict_resolutions(&[low_confidence.clone(), high_confidence.clone()]);
        let steward = ConflictResolutionSteward::new(steward()?);

        let (proposals, kernel) = steward.propose_and_record_to_kernel(
            scan,
            created_at(),
            &ProposalPolicy::strict(),
            continuitydb_memory::MemoryKernel::default(),
        )?;

        let audit_cells = kernel.lookup_cells(CellLookup {
            semantic_anchor: Some("continuitydb:steward:proposal-audit".to_string()),
            ..CellLookup::default()
        })?;
        assert_eq!(proposals.len(), 1);
        assert_eq!(audit_cells.len(), 1);
        assert!(matches!(audit_cells[0].payload, CellPayload::Json(_)));
        Ok(())
    }
}
