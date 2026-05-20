//! Deterministic Steward proposal substrate.

mod error;
mod frontier;
mod ledger;
#[cfg(feature = "local-model")]
mod local_model;
mod mock;
mod policy;
mod proposal;

pub use error::StewardError;
pub use frontier::{FrontierSteward, FrontierWatchEvent, FrontierWatchSignal};
pub use ledger::{ProposalAuditRecord, ProposalLedger};
#[cfg(feature = "local-model")]
pub use local_model::{
    small_model_candidates, LocalModelBackend, LocalModelRequest, LocalModelSteward,
    LocalModelStewardInput, SmallModelCandidate, StewardEvaluationCase,
    StewardEvaluationCaseReport, StewardEvaluationFailure, StewardEvaluationReport,
    StewardEvaluationSuite,
};
pub use mock::{MockSteward, MockStewardInput, MockStewardRule};
pub use policy::{ProposalDecision, ProposalOutcome, ProposalPolicy};
pub use proposal::{ProposalId, StewardAction, StewardIdentity, StewardProposal};

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use continuitydb_core::{SemanticAnchor, StateCellId};
    use continuitydb_revision::RevisionLinkKind;
    #[cfg(feature = "local-model")]
    use std::cell::RefCell;

    #[cfg(feature = "local-model")]
    use super::{
        small_model_candidates, StewardEvaluationCase, StewardEvaluationFailure,
        StewardEvaluationSuite,
    };
    use super::{FrontierSteward, FrontierWatchEvent, FrontierWatchSignal};
    #[cfg(feature = "local-model")]
    use super::{LocalModelBackend, LocalModelRequest, LocalModelSteward, LocalModelStewardInput};
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

    #[cfg(feature = "local-model")]
    #[derive(Debug)]
    struct StaticLocalModelBackend {
        response: String,
        requests: RefCell<Vec<LocalModelRequest>>,
    }

    #[cfg(feature = "local-model")]
    impl StaticLocalModelBackend {
        fn new(response: String) -> Self {
            Self {
                response,
                requests: RefCell::new(Vec::new()),
            }
        }
    }

    #[cfg(feature = "local-model")]
    impl LocalModelBackend for StaticLocalModelBackend {
        fn infer(&self, request: LocalModelRequest) -> Result<String, StewardError> {
            self.requests.borrow_mut().push(request);
            Ok(self.response.clone())
        }
    }

    #[cfg(feature = "local-model")]
    #[test]
    fn local_model_steward_invokes_backend_once() -> Result<(), Box<dyn std::error::Error>> {
        let cell_id = StateCellId::new();
        let response = serde_json::json!({
            "proposals": [{
                "action": {
                    "type": "mark_frontier",
                    "cell_id": cell_id,
                },
                "rationale": "The cell is high impact and stale.",
                "citations": ["test://model-evidence"]
            }]
        })
        .to_string();
        let backend = StaticLocalModelBackend::new(response);
        let steward = LocalModelSteward::new(steward()?, backend);
        let input = LocalModelStewardInput::new(created_at(), "find frontier cells")
            .with_evidence("test://model-evidence", "The cell has not been refreshed.");

        let proposals = steward.propose(input)?;

        assert_eq!(proposals.len(), 1);
        assert!(matches!(
            proposals[0].action(),
            StewardAction::MarkFrontier { cell_id: actual } if *actual == cell_id
        ));
        assert_eq!(steward.backend().requests.borrow().len(), 1);
        assert_eq!(
            steward.backend().requests.borrow()[0].task(),
            "find frontier cells"
        );
        Ok(())
    }

    #[cfg(feature = "local-model")]
    #[test]
    fn local_model_steward_decodes_multiple_json_proposals(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let source = StateCellId::new();
        let target = StateCellId::new();
        let response = serde_json::json!({
            "proposals": [
                {
                    "action": {
                        "type": "link_revision",
                        "source": source,
                        "kind": "conflicts_with",
                        "target": target,
                    },
                    "rationale": "The two cells make incompatible claims.",
                    "citations": ["test://conflict"]
                },
                {
                    "action": {
                        "type": "request_verification",
                        "cell_id": target,
                        "request": "Refresh the target evidence."
                    },
                    "rationale": "The evidence is stale.",
                    "citations": ["test://stale"]
                }
            ]
        })
        .to_string();
        let steward = LocalModelSteward::new(steward()?, StaticLocalModelBackend::new(response));

        let proposals = steward.propose(LocalModelStewardInput::new(
            created_at(),
            "classify revision work",
        ))?;

        assert_eq!(proposals.len(), 2);
        assert!(matches!(
            proposals[0].action(),
            StewardAction::LinkRevision {
                source: actual_source,
                kind: RevisionLinkKind::ConflictsWith,
                target: actual_target,
            } if *actual_source == source && *actual_target == target
        ));
        assert!(matches!(
            proposals[1].action(),
            StewardAction::RequestVerification { cell_id, request }
                if *cell_id == Some(target) && request == "Refresh the target evidence."
        ));
        Ok(())
    }

    #[cfg(feature = "local-model")]
    #[test]
    fn local_model_steward_rejects_invalid_json() -> Result<(), Box<dyn std::error::Error>> {
        let steward = LocalModelSteward::new(
            steward()?,
            StaticLocalModelBackend::new("{not valid json".to_string()),
        );

        let result = steward.propose(LocalModelStewardInput::new(created_at(), "bad output"));

        assert!(matches!(result, Err(StewardError::InvalidModelResponse)));
        Ok(())
    }

    #[cfg(feature = "local-model")]
    #[test]
    fn steward_evaluation_passes_fixed_quality_case() -> Result<(), Box<dyn std::error::Error>> {
        let source = StateCellId::new();
        let target = StateCellId::new();
        let response = serde_json::json!({
            "proposals": [{
                "action": {
                    "type": "link_revision",
                    "source": source,
                    "kind": "conflicts_with",
                    "target": target,
                },
                "rationale": "The evidence directly contradicts the target claim.",
                "citations": ["test://conflict-evidence"]
            }]
        })
        .to_string();
        let steward = LocalModelSteward::new(steward()?, StaticLocalModelBackend::new(response));
        let suite = StewardEvaluationSuite::new(vec![StewardEvaluationCase::new(
            "conflict classification",
            created_at(),
            "classify relation",
        )
        .with_evidence(
            "test://conflict-evidence",
            "Source says shipped; target says blocked.",
        )
        .expect_action(StewardAction::LinkRevision {
            source,
            kind: RevisionLinkKind::ConflictsWith,
            target,
        })
        .require_citation("test://conflict-evidence")
        .forbid_rationale_term("verified in production")]);

        let report = suite.evaluate(&steward);

        assert!(report.passed());
        assert_eq!(report.case_reports().len(), 1);
        assert!(report.case_reports()[0].passed());
        assert!(report.case_reports()[0].failures().is_empty());
        Ok(())
    }

    #[cfg(feature = "local-model")]
    #[test]
    fn steward_evaluation_reports_deterministic_failures() -> Result<(), Box<dyn std::error::Error>>
    {
        let cell_id = StateCellId::new();
        let response = serde_json::json!({
            "proposals": [{
                "action": {
                    "type": "mark_frontier",
                    "cell_id": cell_id,
                },
                "rationale": "This was verified in production by an operator.",
                "citations": ["test://other-evidence"]
            }]
        })
        .to_string();
        let steward = LocalModelSteward::new(steward()?, StaticLocalModelBackend::new(response));
        let suite = StewardEvaluationSuite::new(vec![StewardEvaluationCase::new(
            "unsupported claim",
            created_at(),
            "find uncertainty",
        )
        .with_evidence("test://frontier-evidence", "The cell has stale evidence.")
        .expect_action(StewardAction::MarkFrontier { cell_id })
        .require_citation("test://frontier-evidence")
        .forbid_rationale_term("verified in production")]);

        let report = suite.evaluate(&steward);

        assert!(!report.passed());
        assert_eq!(
            report.case_reports()[0].failures(),
            &[
                StewardEvaluationFailure::MissingCitation {
                    locator: "test://frontier-evidence".to_string(),
                },
                StewardEvaluationFailure::UnsupportedRationaleTerm {
                    term: "verified in production".to_string(),
                },
            ]
        );
        Ok(())
    }

    #[cfg(feature = "local-model")]
    #[test]
    fn steward_evaluation_reports_policy_rejection() -> Result<(), Box<dyn std::error::Error>> {
        let cell_id = StateCellId::new();
        let response = serde_json::json!({
            "proposals": [{
                "action": {
                    "type": "adjust_confidence",
                    "cell_id": cell_id,
                    "proposed_confidence": 1.4
                },
                "rationale": "The confidence should increase.",
                "citations": ["test://confidence"]
            }]
        })
        .to_string();
        let steward = LocalModelSteward::new(steward()?, StaticLocalModelBackend::new(response));
        let suite = StewardEvaluationSuite::new(vec![StewardEvaluationCase::new(
            "invalid confidence",
            created_at(),
            "score confidence",
        )
        .with_evidence("test://confidence", "Evidence is weak.")
        .require_citation("test://confidence")]);

        let report = suite.evaluate(&steward);

        assert_eq!(
            report.case_reports()[0].failures(),
            &[StewardEvaluationFailure::PolicyRejected {
                reasons: vec!["policy:invalid-confidence".to_string()],
            }]
        );
        Ok(())
    }

    #[cfg(feature = "local-model")]
    #[test]
    fn small_model_candidates_include_default_feasibility_model() {
        let candidates = small_model_candidates();

        assert_eq!(candidates[0].model_id(), "Qwen/Qwen2.5-0.5B-Instruct");
        assert_eq!(candidates[0].role(), "default-feasibility");
        assert!(candidates.iter().any(|candidate| {
            candidate.model_id() == "HuggingFaceTB/SmolLM2-360M-Instruct"
                && candidate.role() == "ultra-small-experimental"
        }));
    }

    #[test]
    fn frontier_watch_stale_evidence_emits_verification_request(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let cell_id = StateCellId::new();
        let steward = FrontierSteward::new(steward()?);
        let proposals = steward.propose(
            vec![FrontierWatchEvent::new(
                cell_id,
                FrontierWatchSignal::StaleEvidence,
                "test://stale",
                created_at(),
            )],
            created_at(),
        )?;

        assert_eq!(proposals.len(), 1);
        assert!(matches!(
            proposals[0].action(),
            StewardAction::RequestVerification { cell_id: actual, request }
                if actual == &Some(cell_id) && request == "Refresh stale evidence for frontier cell."
        ));
        assert_eq!(proposals[0].citations(), &["test://stale".to_string()]);
        Ok(())
    }

    #[test]
    fn frontier_watch_high_impact_uncertainty_marks_frontier(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let cell_id = StateCellId::new();
        let steward = FrontierSteward::new(steward()?);
        let proposals = steward.propose(
            vec![FrontierWatchEvent::new(
                cell_id,
                FrontierWatchSignal::HighImpactUncertainty,
                "test://uncertain",
                created_at(),
            )],
            created_at(),
        )?;

        assert_eq!(proposals.len(), 1);
        assert!(matches!(
            proposals[0].action(),
            StewardAction::MarkFrontier { cell_id: actual } if actual == &cell_id
        ));
        assert_eq!(proposals[0].citations(), &["test://uncertain".to_string()]);
        Ok(())
    }

    #[test]
    fn frontier_watch_benign_event_emits_no_proposals() -> Result<(), Box<dyn std::error::Error>> {
        let steward = FrontierSteward::new(steward()?);
        let proposals = steward.propose(
            vec![FrontierWatchEvent::new(
                StateCellId::new(),
                FrontierWatchSignal::Benign,
                "test://current",
                created_at(),
            )],
            created_at(),
        )?;

        assert!(proposals.is_empty());
        Ok(())
    }

    #[test]
    fn frontier_watch_proposals_pass_policy_and_ledger() -> Result<(), Box<dyn std::error::Error>> {
        let steward = FrontierSteward::new(steward()?);
        let proposals = steward.propose(
            vec![
                FrontierWatchEvent::new(
                    StateCellId::new(),
                    FrontierWatchSignal::StaleEvidence,
                    "test://stale",
                    created_at(),
                ),
                FrontierWatchEvent::new(
                    StateCellId::new(),
                    FrontierWatchSignal::HighImpactUncertainty,
                    "test://uncertain",
                    created_at(),
                ),
            ],
            created_at(),
        )?;
        let policy = ProposalPolicy::strict();
        let mut ledger = ProposalLedger::default();

        for proposal in proposals {
            let decision = policy.evaluate(&proposal, created_at());
            assert_eq!(decision.outcome(), ProposalOutcome::Accepted);
            ledger.record(proposal, decision)?;
        }

        assert_eq!(ledger.records().len(), 2);
        Ok(())
    }
}
