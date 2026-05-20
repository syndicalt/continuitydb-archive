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

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use continuitydb_core::{SemanticAnchor, StateCellId};
    use continuitydb_revision::RevisionLinkKind;

    use super::{
        MockSteward, MockStewardInput, MockStewardRule, ProposalDecision, ProposalId,
        ProposalLedger, ProposalOutcome, ProposalPolicy, StewardAction, StewardError,
        StewardIdentity, StewardProposal,
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
    fn proposal_carries_confidence_and_answerability_intent(
    ) -> Result<(), Box<dyn std::error::Error>> {
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

    #[test]
    fn strict_policy_accepts_structurally_valid_link_revision(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let proposal = valid_link_revision()?;
        let decision = ProposalPolicy::strict().evaluate(&proposal, created_at());

        assert_eq!(decision.proposal_id(), proposal.id());
        assert_eq!(decision.outcome(), ProposalOutcome::Accepted);
        assert_eq!(
            decision.reasons(),
            &["policy:structurally-valid".to_string()]
        );
        Ok(())
    }

    #[test]
    fn strict_policy_rejects_empty_answerability_questions(
    ) -> Result<(), Box<dyn std::error::Error>> {
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
        assert_eq!(
            decision.reasons(),
            &["policy:empty-answerability".to_string()]
        );
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
    fn strict_policy_rejects_invalid_confidence_adjustment(
    ) -> Result<(), Box<dyn std::error::Error>> {
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
        assert_eq!(
            decision.reasons(),
            &["policy:invalid-confidence".to_string()]
        );
        Ok(())
    }

    #[test]
    fn ledger_preserves_accepted_and_rejected_proposals() -> Result<(), Box<dyn std::error::Error>>
    {
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
            ledger
                .record_by_id(rejected.id())
                .map(|record| record.proposal().id()),
            Some(rejected.id())
        );
        Ok(())
    }

    #[test]
    fn ledger_rejects_mismatched_proposal_and_decision_ids(
    ) -> Result<(), Box<dyn std::error::Error>> {
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

    #[test]
    fn mock_steward_returns_no_proposals_for_empty_input() -> Result<(), Box<dyn std::error::Error>>
    {
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
        let proposals = mock.propose(MockStewardInput::new(created_at()).with_rule(
            MockStewardRule::MarkFrontier {
                cell_id,
                rationale: "Evidence is stale and high impact.".to_string(),
                citations: vec!["test://frontier".to_string()],
            },
        ))?;

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
    fn mock_steward_emits_link_revision_preserving_fields() -> Result<(), Box<dyn std::error::Error>>
    {
        let source = StateCellId::new();
        let target = StateCellId::new();
        let mock = MockSteward::new(steward()?);
        let proposals = mock.propose(MockStewardInput::new(created_at()).with_rule(
            MockStewardRule::LinkRevision {
                source,
                kind: RevisionLinkKind::ConflictsWith,
                target,
                rationale: "The two cells make incompatible claims.".to_string(),
                citations: vec!["test://conflict".to_string()],
            },
        ))?;

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
    fn mock_steward_reuses_proposal_constructor_validation(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mock = MockSteward::new(steward()?);
        let result = mock.propose(MockStewardInput::new(created_at()).with_rule(
            MockStewardRule::MarkFrontier {
                cell_id: StateCellId::new(),
                rationale: " ".to_string(),
                citations: vec!["test://frontier".to_string()],
            },
        ));

        assert!(matches!(result, Err(StewardError::EmptyRationale)));
        Ok(())
    }

    #[test]
    fn mock_steward_output_can_be_evaluated_by_policy() -> Result<(), Box<dyn std::error::Error>> {
        let mock = MockSteward::new(steward()?);
        let proposals = mock.propose(MockStewardInput::new(created_at()).with_rule(
            MockStewardRule::RequestVerification {
                cell_id: None,
                request: "Verify the newest release evidence.".to_string(),
                rationale: "Release status depends on external evidence.".to_string(),
                citations: vec!["test://release-evidence".to_string()],
            },
        ))?;
        let decision = ProposalPolicy::strict().evaluate(&proposals[0], created_at());

        assert_eq!(decision.proposal_id(), proposals[0].id());
        assert_eq!(decision.outcome(), ProposalOutcome::Accepted);
        Ok(())
    }

    #[test]
    fn mock_steward_output_can_be_recorded_in_ledger() -> Result<(), Box<dyn std::error::Error>> {
        let mock = MockSteward::new(steward()?);
        let proposals = mock.propose(MockStewardInput::new(created_at()).with_rule(
            MockStewardRule::AdjustConfidence {
                cell_id: StateCellId::new(),
                proposed_confidence: 0.65,
                rationale: "New evidence lowers confidence.".to_string(),
                citations: vec!["test://confidence-evidence".to_string()],
            },
        ))?;
        let policy = ProposalPolicy::strict();
        let decision = policy.evaluate(&proposals[0], created_at());
        let mut ledger = ProposalLedger::default();

        ledger.record(proposals[0].clone(), decision)?;

        let records = ledger.records();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].proposal().id(), proposals[0].id());
        assert_eq!(records[0].decision().outcome(), ProposalOutcome::Accepted);
        Ok(())
    }
}
