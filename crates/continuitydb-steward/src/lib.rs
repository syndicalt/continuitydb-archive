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
}
