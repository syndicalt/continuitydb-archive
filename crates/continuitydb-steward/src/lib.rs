//! Deterministic Steward proposal substrate.

mod conflict;
mod error;
mod frontier;
mod ledger;
#[cfg(feature = "local-model")]
mod local_model;
mod mock;
mod policy;
mod proposal;

pub use conflict::ConflictResolutionSteward;
pub use error::StewardError;
pub use frontier::{
    FileFrontierSubscriptionStore, FrontierSteward, FrontierSubscription, FrontierSubscriptionId,
    FrontierSubscriptionRunner, FrontierSubscriptionStore, FrontierWatchEvent, FrontierWatchSignal,
    MemoryFrontierSubscriptionStore,
};
pub use ledger::{
    BorrowedKernelProposalStore, FileProposalStore, KernelProposalStore, MemoryProposalStore,
    ProposalAuditRecord, ProposalLedger, ProposalLedgerStore, StoredProposalLedger,
};
#[cfg(feature = "local-model")]
pub use local_model::{
    default_steward_evaluation_suite, latest_compatible_local_model_benchmark_baseline,
    latest_local_model_benchmark_baseline, local_model_prompt_fingerprint_for_suite,
    local_model_prompt_for_input, local_model_response_gbnf_grammar,
    local_model_response_json_schema, record_local_model_benchmark_baseline,
    record_local_model_benchmark_baseline_with_regression, small_model_candidates,
    FileLocalModelBenchmarkBaselineStore, LlamaCppRuntimeProfile, LocalExecutableRunner,
    LocalExecutableRunnerConfig, LocalModelBackend, LocalModelBenchmark,
    LocalModelBenchmarkBaseline, LocalModelBenchmarkBaselineStore, LocalModelBenchmarkGateReport,
    LocalModelBenchmarkRegression, LocalModelBenchmarkReport, LocalModelRequest,
    LocalModelResponseFingerprint, LocalModelRuntimeManifest, LocalModelStabilityCaseReport,
    LocalModelStabilityReport, LocalModelSteward, LocalModelStewardInput,
    MemoryLocalModelBenchmarkBaselineStore, MistralRsRuntimeProfile, SmallModelCandidate,
    StewardEvaluationCase, StewardEvaluationCaseReport, StewardEvaluationCaseResponse,
    StewardEvaluationFailure, StewardEvaluationReport, StewardEvaluationSuite,
    StewardEvaluationSummary, LOCAL_MODEL_RESPONSE_SCHEMA_VERSION,
};
pub use mock::{MockSteward, MockStewardInput, MockStewardRule};
pub use policy::{ProposalDecision, ProposalOutcome, ProposalPolicy};
pub use proposal::{ProposalId, StewardAction, StewardIdentity, StewardProposal};

#[cfg(test)]
mod tests {
    #[cfg(feature = "local-model")]
    use super::{
        default_steward_evaluation_suite, latest_compatible_local_model_benchmark_baseline,
        latest_local_model_benchmark_baseline, local_model_prompt_for_input,
        local_model_response_gbnf_grammar, local_model_response_json_schema,
        record_local_model_benchmark_baseline,
        record_local_model_benchmark_baseline_with_regression, small_model_candidates,
        FileLocalModelBenchmarkBaselineStore, LlamaCppRuntimeProfile, LocalModelBenchmark,
        LocalModelBenchmarkBaseline, LocalModelBenchmarkBaselineStore,
        LocalModelBenchmarkGateReport, LocalModelBenchmarkRegression,
        MemoryLocalModelBenchmarkBaselineStore, MistralRsRuntimeProfile, SmallModelCandidate,
        StewardEvaluationCase, StewardEvaluationFailure, StewardEvaluationSuite,
        StewardEvaluationSummary, LOCAL_MODEL_RESPONSE_SCHEMA_VERSION,
    };
    use super::{
        BorrowedKernelProposalStore, FileProposalStore, KernelProposalStore, MemoryProposalStore,
        MockSteward, MockStewardInput, MockStewardRule, ProposalDecision, ProposalId,
        ProposalLedger, ProposalLedgerStore, ProposalOutcome, ProposalPolicy, StewardAction,
        StewardError, StewardIdentity, StewardProposal, StoredProposalLedger,
    };
    use super::{
        FileFrontierSubscriptionStore, FrontierSteward, FrontierSubscription,
        FrontierSubscriptionId, FrontierSubscriptionRunner, FrontierSubscriptionStore,
        FrontierWatchEvent, FrontierWatchSignal, MemoryFrontierSubscriptionStore,
    };
    #[cfg(feature = "local-model")]
    use super::{
        LocalExecutableRunner, LocalExecutableRunnerConfig, LocalModelBackend, LocalModelRequest,
        LocalModelSteward, LocalModelStewardInput,
    };
    use chrono::{TimeZone, Utc};
    use continuitydb_core::{SemanticAnchor, StateCellId};
    use continuitydb_revision::RevisionLinkKind;
    #[cfg(feature = "local-model")]
    use std::cell::RefCell;
    #[cfg(feature = "local-model")]
    use std::path::Path;
    use std::{fs, path::PathBuf};

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

    fn temp_proposal_store_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("{name}-{:?}.jsonl", ProposalId::new()))
    }

    fn temp_frontier_subscription_store_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("{name}-{:?}.jsonl", ProposalId::new()))
    }

    #[cfg(feature = "local-model")]
    fn temp_local_model_baseline_store_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("{name}-{:?}.jsonl", ProposalId::new()))
    }

    #[cfg(feature = "local-model")]
    fn local_model_baseline_for_response(
        cell_id: StateCellId,
        response: String,
    ) -> Result<LocalModelBenchmarkBaseline, Box<dyn std::error::Error>> {
        Ok(LocalModelBenchmarkBaseline::from_report(
            local_model_benchmark_for_response(cell_id, response)?.run(steward()?),
            created_at(),
        ))
    }

    #[cfg(feature = "local-model")]
    fn local_model_benchmark_for_response(
        cell_id: StateCellId,
        response: String,
    ) -> Result<LocalModelBenchmark, Box<dyn std::error::Error>> {
        let script = write_local_model_script(
            "continuitydb-local-model-baseline-regression",
            &format!("cat >/dev/null\nprintf '%s\\n' '{response}'\n"),
        )?;
        let runner = LocalExecutableRunner::new(
            LocalExecutableRunnerConfig::new("sh").with_argument(script),
        );
        let suite = StewardEvaluationSuite::new(vec![StewardEvaluationCase::new(
            "frontier baseline regression",
            created_at(),
            "mark frontier",
        )
        .with_evidence("test://frontier", "Evidence is stale.")
        .expect_action(StewardAction::MarkFrontier { cell_id })
        .require_citation("test://frontier")]);
        Ok(LocalModelBenchmark::new(
            small_model_candidates()[0],
            runner,
            suite,
        ))
    }

    #[cfg(feature = "local-model")]
    fn local_model_empty_baseline(
        candidate: SmallModelCandidate,
        recorded_at: chrono::DateTime<Utc>,
    ) -> Result<LocalModelBenchmarkBaseline, StewardError> {
        Ok(LocalModelBenchmarkBaseline::from_report(
            LocalModelBenchmark::new(
                candidate,
                LocalExecutableRunner::new(LocalExecutableRunnerConfig::new("sh")),
                StewardEvaluationSuite::new(Vec::new()),
            )
            .run(steward()?),
            recorded_at,
        ))
    }

    #[cfg(feature = "local-model")]
    fn local_model_runtime_baseline(
        candidate: SmallModelCandidate,
        recorded_at: chrono::DateTime<Utc>,
        runtime_argument: &str,
    ) -> Result<LocalModelBenchmarkBaseline, StewardError> {
        Ok(LocalModelBenchmarkBaseline::from_report(
            LocalModelBenchmark::new(
                candidate,
                LocalExecutableRunner::new(
                    LocalExecutableRunnerConfig::new("sh").with_argument(runtime_argument),
                ),
                StewardEvaluationSuite::new(Vec::new()),
            )
            .run(steward()?),
            recorded_at,
        ))
    }

    #[cfg(feature = "local-model")]
    fn local_model_suite_baseline(
        candidate: SmallModelCandidate,
        recorded_at: chrono::DateTime<Utc>,
        suite: StewardEvaluationSuite,
    ) -> Result<LocalModelBenchmarkBaseline, StewardError> {
        Ok(LocalModelBenchmarkBaseline::from_report(
            LocalModelBenchmark::new(
                candidate,
                LocalExecutableRunner::new(LocalExecutableRunnerConfig::new("sh")),
                suite,
            )
            .run(steward()?),
            recorded_at,
        ))
    }

    #[cfg(feature = "local-model")]
    fn local_model_file_backed_benchmark(
        cell_id: StateCellId,
        script_name: &str,
        response_path: &Path,
    ) -> Result<LocalModelBenchmark, Box<dyn std::error::Error>> {
        let script = write_local_model_script(
            script_name,
            &format!("cat >/dev/null\ncat '{}'\n", response_path.display()),
        )?;
        let runner = LocalExecutableRunner::new(
            LocalExecutableRunnerConfig::new("sh").with_argument(script),
        );
        let suite = StewardEvaluationSuite::new(vec![StewardEvaluationCase::new(
            "frontier baseline regression",
            created_at(),
            "mark frontier",
        )
        .with_evidence("test://frontier", "Evidence is stale.")
        .expect_action(StewardAction::MarkFrontier { cell_id })
        .require_citation("test://frontier")]);
        Ok(LocalModelBenchmark::new(
            small_model_candidates()[0],
            runner,
            suite,
        ))
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
    #[derive(Debug)]
    struct FailingLocalModelBackend;

    #[cfg(feature = "local-model")]
    impl LocalModelBackend for FailingLocalModelBackend {
        fn infer(&self, _request: LocalModelRequest) -> Result<String, StewardError> {
            Err(StewardError::LocalModelExecutionFailed)
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
    fn local_model_prompt_helper_matches_backend_prompt() -> Result<(), Box<dyn std::error::Error>>
    {
        let response = r#"{"proposals":[]}"#.to_string();
        let steward = LocalModelSteward::new(steward()?, StaticLocalModelBackend::new(response));
        let input = LocalModelStewardInput::new(created_at(), "find uncertain cells")
            .with_evidence("test://evidence", "Evidence text.");
        let expected_prompt = local_model_prompt_for_input(&input);

        let _ = steward.propose(input)?;

        assert_eq!(steward.backend().requests.borrow().len(), 1);
        assert_eq!(
            steward.backend().requests.borrow()[0].prompt(),
            expected_prompt
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
    fn steward_evaluation_reports_model_execution_failure() -> Result<(), Box<dyn std::error::Error>>
    {
        let steward = LocalModelSteward::new(steward()?, FailingLocalModelBackend);
        let suite = StewardEvaluationSuite::new(vec![StewardEvaluationCase::new(
            "execution failure",
            created_at(),
            "run local model",
        )]);

        let report = suite.evaluate(&steward);

        assert_eq!(
            report.case_reports()[0].failures(),
            &[StewardEvaluationFailure::ModelExecutionFailed]
        );
        Ok(())
    }

    #[cfg(feature = "local-model")]
    #[test]
    fn steward_evaluation_reports_invalid_model_response() -> Result<(), Box<dyn std::error::Error>>
    {
        let steward = LocalModelSteward::new(
            steward()?,
            StaticLocalModelBackend::new("{not valid json".to_string()),
        );
        let suite = StewardEvaluationSuite::new(vec![StewardEvaluationCase::new(
            "invalid response",
            created_at(),
            "decode local model output",
        )]);

        let report = suite.evaluate(&steward);

        assert_eq!(
            report.case_reports()[0].failures(),
            &[StewardEvaluationFailure::InvalidModelResponse]
        );
        Ok(())
    }

    #[cfg(feature = "local-model")]
    #[test]
    fn steward_evaluation_failure_serializes_stable_failure_codes(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let execution = serde_json::to_value(StewardEvaluationFailure::ModelExecutionFailed)?;
        let invalid = serde_json::to_value(StewardEvaluationFailure::InvalidModelResponse)?;
        let missing = serde_json::to_value(StewardEvaluationFailure::MissingCitation {
            locator: "test://evidence".to_string(),
        })?;

        assert_eq!(execution, serde_json::json!("model_execution_failed"));
        assert_eq!(invalid, serde_json::json!("invalid_model_response"));
        assert_eq!(
            missing,
            serde_json::json!({
                "missing_citation": {
                    "locator": "test://evidence"
                }
            })
        );
        Ok(())
    }

    #[cfg(feature = "local-model")]
    #[test]
    fn steward_evaluation_failure_decodes_legacy_failure_names(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let execution: StewardEvaluationFailure =
            serde_json::from_value(serde_json::json!("ModelExecutionFailed"))?;
        let invalid: StewardEvaluationFailure =
            serde_json::from_value(serde_json::json!("InvalidModelResponse"))?;
        let missing: StewardEvaluationFailure = serde_json::from_value(serde_json::json!({
            "MissingCitation": {
                "locator": "test://evidence"
            }
        }))?;

        assert_eq!(execution, StewardEvaluationFailure::ModelExecutionFailed);
        assert_eq!(invalid, StewardEvaluationFailure::InvalidModelResponse);
        assert_eq!(
            missing,
            StewardEvaluationFailure::MissingCitation {
                locator: "test://evidence".to_string(),
            }
        );
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
    fn steward_evaluation_summary_counts_passed_and_failed_cases(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let passing_cell = StateCellId::new();
        let missing_cell = StateCellId::new();
        let response = serde_json::json!({
            "proposals": [{
                "action": {
                    "type": "mark_frontier",
                    "cell_id": passing_cell,
                },
                "rationale": "The supplied evidence is stale.",
                "citations": ["test://frontier"]
            }]
        })
        .to_string();
        let steward = LocalModelSteward::new(steward()?, StaticLocalModelBackend::new(response));
        let suite = StewardEvaluationSuite::new(vec![
            StewardEvaluationCase::new("passes", created_at(), "find frontier")
                .with_evidence("test://frontier", "Evidence is stale.")
                .expect_action(StewardAction::MarkFrontier {
                    cell_id: passing_cell,
                })
                .require_citation("test://frontier"),
            StewardEvaluationCase::new("fails", created_at(), "find other frontier")
                .with_evidence("test://missing", "Other evidence is stale.")
                .expect_action(StewardAction::MarkFrontier {
                    cell_id: missing_cell,
                })
                .require_citation("test://missing"),
        ]);

        let summary: StewardEvaluationSummary = suite.evaluate(&steward).summary();

        assert_eq!(summary.total_cases(), 2);
        assert_eq!(summary.passed_cases(), 1);
        assert_eq!(summary.failed_cases(), 1);
        assert_eq!(summary.pass_rate(), 0.5);
        assert!(!summary.passed());
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
    fn steward_evaluation_passes_required_rationale_term() -> Result<(), Box<dyn std::error::Error>>
    {
        let cell_id = StateCellId::new();
        let response = serde_json::json!({
            "proposals": [{
                "action": {
                    "type": "request_verification",
                    "cell_id": cell_id,
                    "request": "Gather additional source evidence."
                },
                "rationale": "The evidence is insufficient, so uncertainty remains.",
                "citations": ["test://thin-evidence"]
            }]
        })
        .to_string();
        let steward = LocalModelSteward::new(steward()?, StaticLocalModelBackend::new(response));
        let suite = StewardEvaluationSuite::new(vec![StewardEvaluationCase::new(
            "insufficient evidence uncertainty",
            created_at(),
            "explain uncertainty",
        )
        .with_evidence(
            "test://thin-evidence",
            "One weak source mentions the claim.",
        )
        .expect_action(StewardAction::RequestVerification {
            cell_id: Some(cell_id),
            request: "Gather additional source evidence.".to_string(),
        })
        .require_citation("test://thin-evidence")
        .require_rationale_term("uncertainty")]);

        let report = suite.evaluate(&steward);

        assert!(report.passed());
        Ok(())
    }

    #[cfg(feature = "local-model")]
    #[test]
    fn default_steward_evaluation_suite_scores_valid_verification_proposal(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let source = StateCellId::from_u128(1);
        let target = StateCellId::from_u128(2);
        let supersession_source = StateCellId::from_u128(4);
        let supersession_target = StateCellId::from_u128(5);
        let confidence_cell = StateCellId::from_u128(6);
        let targeted_verification_cell = StateCellId::from_u128(7);
        let response = serde_json::json!({
            "proposals": [
                {
                    "action": {
                        "type": "request_verification",
                        "cell_id": null,
                        "request": "Gather additional source evidence."
                    },
                    "rationale": "The evidence is thin, so uncertainty remains.",
                    "citations": ["continuitydb://evaluation/thin-evidence"]
                },
                {
                    "action": {
                        "type": "link_revision",
                        "source": source,
                        "kind": "conflicts_with",
                        "target": target
                    },
                    "rationale": "The cited evidence directly contradicts the target claim.",
                    "citations": ["continuitydb://evaluation/conflict-evidence"]
                },
                {
                    "action": {
                        "type": "link_revision",
                        "source": supersession_source,
                        "kind": "supersedes",
                        "target": supersession_target
                    },
                    "rationale": "The newer evidence supersedes the older status without contradicting it.",
                    "citations": ["continuitydb://evaluation/supersession-evidence"]
                },
                {
                    "action": {
                        "type": "request_verification",
                        "cell_id": null,
                        "request": "Verify deployment status before treating the release as shipped."
                    },
                    "rationale": "The evidence does not support deployment, so the shipped claim remains unsupported.",
                    "citations": ["continuitydb://evaluation/unsupported-release-claim"]
                },
                {
                    "action": {
                        "type": "adjust_confidence",
                        "cell_id": confidence_cell,
                        "proposed_confidence": 0.42
                    },
                    "rationale": "The cited evidence lowers confidence in the stale deployment status.",
                    "citations": ["continuitydb://evaluation/confidence-evidence"]
                },
                {
                    "action": {
                        "type": "request_verification",
                        "cell_id": targeted_verification_cell,
                        "request": "Refresh the stale high-impact frontier signal."
                    },
                    "rationale": "The stale high-impact frontier signal needs a refresh from current evidence.",
                    "citations": ["continuitydb://evaluation/targeted-verification-evidence"]
                },
                {
                    "action": {
                        "type": "create_cell_draft",
                        "anchors": ["project:continuitydb:benchmark-result"],
                        "payload_text": "ContinuityDB local Steward benchmark produced a new result requiring review."
                    },
                    "rationale": "The new benchmark evidence supports drafting a StateCell for review.",
                    "citations": ["continuitydb://evaluation/new-benchmark-evidence"]
                },
                {
                    "action": {
                        "type": "mark_frontier",
                        "cell_id": "00000000-0000-0000-0000-000000000003"
                    },
                    "rationale": "The release status changed between the build and incident sources, so this state should stay on the frontier.",
                    "citations": [
                        "continuitydb://evaluation/release-build-source",
                        "continuitydb://evaluation/release-incident-source"
                    ]
                },
                {
                    "action": {
                        "type": "request_verification",
                        "cell_id": null,
                        "request": "Ask for a concrete answerability question before labeling the cell."
                    },
                    "rationale": "The answerability label input is invalid because it has no concrete question.",
                    "citations": ["continuitydb://evaluation/invalid-answerability-label"]
                }
            ]
        })
        .to_string();
        let steward = LocalModelSteward::new(steward()?, StaticLocalModelBackend::new(response));

        let report = default_steward_evaluation_suite().evaluate(&steward);

        assert!(report.passed());
        assert_eq!(report.case_reports().len(), 9);
        assert!(report.case_reports()[1].passed());
        assert_eq!(report.case_reports()[1].name(), "conflict classification");
        assert!(report.case_reports()[2].passed());
        assert_eq!(
            report.case_reports()[2].name(),
            "supersession classification"
        );
        assert!(report.case_reports()[3].passed());
        assert_eq!(
            report.case_reports()[3].name(),
            "unsupported claim boundary"
        );
        assert!(report.case_reports()[4].passed());
        assert_eq!(report.case_reports()[4].name(), "confidence adjustment");
        assert!(report.case_reports()[5].passed());
        assert_eq!(
            report.case_reports()[5].name(),
            "targeted verification request"
        );
        assert!(report.case_reports()[6].passed());
        assert_eq!(
            report.case_reports()[6].name(),
            "new evidence draft creation"
        );
        assert!(report.case_reports()[7].passed());
        assert_eq!(
            report.case_reports()[7].name(),
            "multi-source citation preservation"
        );
        assert!(report.case_reports()[8].passed());
        assert_eq!(
            report.case_reports()[8].name(),
            "policy rejection avoidance"
        );
        Ok(())
    }

    #[cfg(feature = "local-model")]
    #[test]
    fn default_steward_evaluation_suite_exposes_case_contracts() {
        let suite = default_steward_evaluation_suite();

        assert_eq!(suite.len(), 9);
        assert!(!suite.is_empty());
        let cases = suite.cases();

        assert_eq!(cases[0].name(), "insufficient evidence uncertainty");
        assert_eq!(
            cases[0].input().task(),
            "Assess whether thin evidence needs verification."
        );
        assert_eq!(
            cases[0].input().evidence()[0].locator(),
            "continuitydb://evaluation/thin-evidence"
        );
        assert_eq!(
            cases[0].required_citations(),
            ["continuitydb://evaluation/thin-evidence".to_string()].as_slice()
        );
        assert_eq!(
            cases[0].required_rationale_terms(),
            ["uncertainty".to_string()].as_slice()
        );
        assert!(matches!(
            &cases[0].expected_actions()[0],
            StewardAction::RequestVerification { cell_id: None, request }
                if request == "Gather additional source evidence."
        ));

        assert_eq!(cases[1].name(), "conflict classification");
        assert_eq!(
            cases[1].input().task(),
            "Classify whether contradictory release-status claims conflict."
        );
        assert_eq!(
            cases[1].input().evidence()[0].locator(),
            "continuitydb://evaluation/conflict-evidence"
        );
        assert_eq!(
            cases[1].forbidden_rationale_terms(),
            ["verified in production".to_string()].as_slice()
        );
        assert!(matches!(
            &cases[1].expected_actions()[0],
            StewardAction::LinkRevision {
                kind: RevisionLinkKind::ConflictsWith,
                ..
            }
        ));

        assert_eq!(cases[2].name(), "supersession classification");
        assert_eq!(
            cases[2].input().task(),
            "Classify whether newer release evidence supersedes the older status."
        );
        assert_eq!(
            cases[2].input().evidence()[0].locator(),
            "continuitydb://evaluation/supersession-evidence"
        );
        assert_eq!(
            cases[2].required_citations(),
            ["continuitydb://evaluation/supersession-evidence".to_string()].as_slice()
        );
        assert_eq!(
            cases[2].required_rationale_terms(),
            ["supersedes".to_string()].as_slice()
        );
        assert_eq!(
            cases[2].forbidden_rationale_terms(),
            ["conflicts with".to_string()].as_slice()
        );
        assert!(matches!(
            &cases[2].expected_actions()[0],
            StewardAction::LinkRevision {
                kind: RevisionLinkKind::Supersedes,
                ..
            }
        ));

        assert_eq!(cases[3].name(), "unsupported claim boundary");
        assert_eq!(
            cases[3].input().task(),
            "Check whether release evidence supports a shipped deployment claim."
        );
        assert_eq!(
            cases[3].input().evidence()[0].locator(),
            "continuitydb://evaluation/unsupported-release-claim"
        );
        assert_eq!(
            cases[3].required_citations(),
            ["continuitydb://evaluation/unsupported-release-claim".to_string()].as_slice()
        );
        assert_eq!(
            cases[3].required_rationale_terms(),
            ["unsupported".to_string()].as_slice()
        );
        assert_eq!(
            cases[3].forbidden_rationale_terms(),
            ["deployed to all customers".to_string()].as_slice()
        );
        assert!(matches!(
            &cases[3].expected_actions()[0],
            StewardAction::RequestVerification { cell_id: None, request }
                if request == "Verify deployment status before treating the release as shipped."
        ));

        assert_eq!(cases[4].name(), "confidence adjustment");
        assert_eq!(
            cases[4].input().task(),
            "Adjust confidence for stale deployment status evidence."
        );
        assert_eq!(
            cases[4].input().evidence()[0].locator(),
            "continuitydb://evaluation/confidence-evidence"
        );
        assert_eq!(
            cases[4].required_citations(),
            ["continuitydb://evaluation/confidence-evidence".to_string()].as_slice()
        );
        assert_eq!(
            cases[4].required_rationale_terms(),
            ["confidence".to_string()].as_slice()
        );
        assert_eq!(
            cases[4].forbidden_rationale_terms(),
            ["fully trusted".to_string()].as_slice()
        );
        assert!(matches!(
            &cases[4].expected_actions()[0],
            StewardAction::AdjustConfidence {
                cell_id,
                proposed_confidence
            } if *cell_id == StateCellId::from_u128(6)
                && (*proposed_confidence - 0.42).abs() < f32::EPSILON
        ));

        assert_eq!(cases[5].name(), "targeted verification request");
        assert_eq!(
            cases[5].input().task(),
            "Request verification for a stale high-impact frontier cell."
        );
        assert_eq!(
            cases[5].input().evidence()[0].locator(),
            "continuitydb://evaluation/targeted-verification-evidence"
        );
        assert_eq!(
            cases[5].required_citations(),
            ["continuitydb://evaluation/targeted-verification-evidence".to_string()].as_slice()
        );
        assert_eq!(
            cases[5].required_rationale_terms(),
            ["refresh".to_string()].as_slice()
        );
        assert_eq!(
            cases[5].forbidden_rationale_terms(),
            ["no target".to_string()].as_slice()
        );
        assert!(matches!(
            &cases[5].expected_actions()[0],
            StewardAction::RequestVerification {
                cell_id: Some(cell_id),
                request
            } if *cell_id == StateCellId::from_u128(7)
                && request == "Refresh the stale high-impact frontier signal."
        ));

        assert_eq!(cases[6].name(), "new evidence draft creation");
        assert_eq!(
            cases[6].input().task(),
            "Draft a StateCell from new benchmark evidence."
        );
        assert_eq!(
            cases[6].input().evidence()[0].locator(),
            "continuitydb://evaluation/new-benchmark-evidence"
        );
        assert_eq!(
            cases[6].required_citations(),
            ["continuitydb://evaluation/new-benchmark-evidence".to_string()].as_slice()
        );
        assert_eq!(
            cases[6].required_rationale_terms(),
            ["draft".to_string()].as_slice()
        );
        assert_eq!(
            cases[6].forbidden_rationale_terms(),
            ["committed".to_string()].as_slice()
        );
        assert!(matches!(
            &cases[6].expected_actions()[0],
            StewardAction::CreateCellDraft {
                anchors,
                payload_text
            } if anchors == &[SemanticAnchor::new("project:continuitydb:benchmark-result")]
                && payload_text
                    == "ContinuityDB local Steward benchmark produced a new result requiring review."
        ));

        assert_eq!(cases[7].name(), "multi-source citation preservation");
        assert_eq!(
            cases[7].input().task(),
            "Decide whether a release-status change should stay on the active frontier."
        );
        assert_eq!(cases[7].input().evidence().len(), 2);
        assert_eq!(
            cases[7].input().evidence()[0].locator(),
            "continuitydb://evaluation/release-build-source"
        );
        assert_eq!(
            cases[7].input().evidence()[1].locator(),
            "continuitydb://evaluation/release-incident-source"
        );
        assert_eq!(
            cases[7].required_citations(),
            [
                "continuitydb://evaluation/release-build-source".to_string(),
                "continuitydb://evaluation/release-incident-source".to_string(),
            ]
            .as_slice()
        );
        assert_eq!(
            cases[7].required_rationale_terms(),
            ["frontier".to_string()].as_slice()
        );
        assert!(matches!(
            &cases[7].expected_actions()[0],
            StewardAction::MarkFrontier { cell_id }
                if *cell_id == StateCellId::from_u128(3)
        ));

        assert_eq!(cases[8].name(), "policy rejection avoidance");
        assert_eq!(
            cases[8].input().task(),
            "Handle invalid answerability-label evidence without emitting an invalid label."
        );
        assert_eq!(
            cases[8].input().evidence()[0].locator(),
            "continuitydb://evaluation/invalid-answerability-label"
        );
        assert_eq!(
            cases[8].required_citations(),
            ["continuitydb://evaluation/invalid-answerability-label".to_string()].as_slice()
        );
        assert_eq!(
            cases[8].required_rationale_terms(),
            ["invalid".to_string()].as_slice()
        );
        assert_eq!(
            cases[8].forbidden_rationale_terms(),
            ["label applied".to_string()].as_slice()
        );
        assert!(matches!(
            &cases[8].expected_actions()[0],
            StewardAction::RequestVerification { cell_id: None, request }
                if request == "Ask for a concrete answerability question before labeling the cell."
        ));
    }

    #[cfg(feature = "local-model")]
    #[test]
    fn steward_evaluation_suite_fingerprint_changes_with_case_contract() {
        let base = StewardEvaluationSuite::new(vec![StewardEvaluationCase::new(
            "frontier",
            created_at(),
            "mark frontier",
        )
        .with_evidence("test://frontier", "Evidence is stale.")
        .expect_action(StewardAction::MarkFrontier {
            cell_id: StateCellId::from_u128(7),
        })
        .require_citation("test://frontier")]);
        let same = base.clone();
        let changed = StewardEvaluationSuite::new(vec![StewardEvaluationCase::new(
            "frontier",
            created_at(),
            "mark frontier",
        )
        .with_evidence("test://frontier", "Evidence is stale.")
        .expect_action(StewardAction::MarkFrontier {
            cell_id: StateCellId::from_u128(7),
        })
        .require_citation("test://different")]);

        assert_eq!(base.fingerprint(), same.fingerprint());
        assert_ne!(base.fingerprint(), changed.fingerprint());
        assert!(base.fingerprint().starts_with("fnv1a64:"));
    }

    #[cfg(feature = "local-model")]
    #[test]
    fn steward_evaluation_suite_captures_raw_case_responses(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let cell_id = StateCellId::new();
        let response = serde_json::json!({
            "proposals": [{
                "action": {
                    "type": "request_verification",
                    "cell_id": cell_id,
                    "request": "Gather additional source evidence."
                },
                "rationale": "The evidence is uncertain and needs another source.",
                "citations": ["test://thin-evidence"]
            }]
        })
        .to_string();
        let steward =
            LocalModelSteward::new(steward()?, StaticLocalModelBackend::new(response.clone()));
        let suite = StewardEvaluationSuite::new(vec![StewardEvaluationCase::new(
            "capture raw response",
            created_at(),
            "explain uncertainty",
        )
        .with_evidence(
            "test://thin-evidence",
            "One weak source mentions the claim.",
        )
        .expect_action(StewardAction::RequestVerification {
            cell_id: Some(cell_id),
            request: "Gather additional source evidence.".to_string(),
        })
        .require_citation("test://thin-evidence")
        .require_rationale_term("uncertain")]);

        let (report, responses) = suite.evaluate_with_responses(&steward);

        assert!(report.passed());
        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].case_name(), "capture raw response");
        assert_eq!(responses[0].response(), Some(response.as_str()));
        assert_eq!(responses[0].response_bytes(), response.len());
        assert_eq!(steward.backend().requests.borrow().len(), 1);
        Ok(())
    }

    #[cfg(feature = "local-model")]
    #[test]
    fn steward_evaluation_reports_missing_required_rationale_term(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let cell_id = StateCellId::new();
        let response = serde_json::json!({
            "proposals": [{
                "action": {
                    "type": "request_verification",
                    "cell_id": cell_id,
                    "request": "Gather additional source evidence."
                },
                "rationale": "The evidence needs another source.",
                "citations": ["test://thin-evidence"]
            }]
        })
        .to_string();
        let steward = LocalModelSteward::new(steward()?, StaticLocalModelBackend::new(response));
        let suite = StewardEvaluationSuite::new(vec![StewardEvaluationCase::new(
            "missing uncertainty language",
            created_at(),
            "explain uncertainty",
        )
        .with_evidence(
            "test://thin-evidence",
            "One weak source mentions the claim.",
        )
        .expect_action(StewardAction::RequestVerification {
            cell_id: Some(cell_id),
            request: "Gather additional source evidence.".to_string(),
        })
        .require_citation("test://thin-evidence")
        .require_rationale_term("uncertainty")]);

        let report = suite.evaluate(&steward);

        assert_eq!(
            report.case_reports()[0].failures(),
            &[StewardEvaluationFailure::MissingRationaleTerm {
                term: "uncertainty".to_string(),
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

    #[cfg(feature = "local-model")]
    #[test]
    fn small_model_candidates_expose_runtime_metadata() {
        let candidates = small_model_candidates();

        assert!(candidates.iter().all(|candidate| {
            !candidate.recommended_runtime().is_empty()
                && !candidate.artifact_format().is_empty()
                && candidate.requires_grammar()
                && !candidate.notes().is_empty()
        }));
        assert_eq!(candidates[0].recommended_runtime(), "llama.cpp");
        assert_eq!(candidates[0].artifact_format(), "GGUF");
        assert_eq!(candidates[0].recommended_temperature(), 0.0);
    }

    #[cfg(feature = "local-model")]
    #[test]
    fn small_model_candidate_builds_recommended_runner_config() {
        let config =
            small_model_candidates()[0].recommended_runner_config("llama-cli", "/models/qwen.gguf");

        assert_eq!(config.executable(), std::path::Path::new("llama-cli"));
        assert_eq!(
            config.command_arguments(),
            vec![
                "--model".to_string(),
                "/models/qwen.gguf".to_string(),
                "--ctx-size".to_string(),
                "4096".to_string(),
                "--temp".to_string(),
                "0".to_string(),
                "--prompt".to_string(),
                "-".to_string(),
            ]
        );
    }

    #[cfg(feature = "local-model")]
    #[test]
    fn local_model_response_json_schema_describes_steward_proposals(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let schema: serde_json::Value = serde_json::from_str(local_model_response_json_schema())?;

        assert_eq!(
            schema["$id"].as_str(),
            Some("https://continuitydb.dev/schemas/local-model-response.schema.json")
        );
        assert_eq!(
            schema["x-continuitydb-schema-version"].as_u64(),
            Some(LOCAL_MODEL_RESPONSE_SCHEMA_VERSION as u64)
        );
        assert_eq!(schema["required"], serde_json::json!(["proposals"]));
        assert!(schema["properties"]["proposals"].is_object());
        assert_eq!(
            schema["$defs"]["action"]["oneOf"][0]["properties"]["type"]["const"].as_str(),
            Some("create_cell_draft")
        );
        assert_eq!(
            schema["$defs"]["action"]["oneOf"][5]["properties"]["type"]["const"].as_str(),
            Some("request_verification")
        );
        Ok(())
    }

    #[cfg(feature = "local-model")]
    #[test]
    fn local_model_response_gbnf_grammar_describes_proposal_shape() {
        let grammar = local_model_response_gbnf_grammar();

        assert!(grammar.contains("root ::= response"));
        assert!(grammar.contains("response ::= object-start ws proposals-field ws object-end"));
        assert!(grammar.contains("proposal ::= object-start ws action-field"));
        assert!(grammar.contains("action ::= create-cell-draft-action"));
        assert!(grammar.contains("request-verification-action"));
    }

    #[cfg(feature = "local-model")]
    fn write_local_model_script(
        name: &str,
        body: &str,
    ) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let path = std::env::temp_dir().join(format!("{name}-{:?}.sh", ProposalId::new()));
        fs::write(&path, body)?;
        Ok(path)
    }

    #[cfg(feature = "local-model")]
    #[test]
    fn local_executable_runner_builds_deterministic_arguments() {
        let config = LocalExecutableRunnerConfig::new("llama-cli")
            .with_model_path("models/qwen2.5-0.5b.gguf")
            .with_argument("--ctx-size")
            .with_argument("4096")
            .with_argument("--json-schema")
            .with_argument("steward-proposal.schema.json");

        assert_eq!(
            config.command_arguments(),
            &[
                "--model".to_string(),
                "models/qwen2.5-0.5b.gguf".to_string(),
                "--ctx-size".to_string(),
                "4096".to_string(),
                "--json-schema".to_string(),
                "steward-proposal.schema.json".to_string(),
            ]
        );
    }

    #[cfg(feature = "local-model")]
    #[test]
    fn local_executable_runner_returns_stdout_response() -> Result<(), Box<dyn std::error::Error>> {
        let script = write_local_model_script(
            "continuitydb-local-model-ok",
            "cat >/dev/null\nprintf '%s\\n' '{\"proposals\":[]}'\n",
        )?;
        let runner = LocalExecutableRunner::new(
            LocalExecutableRunnerConfig::new("sh").with_argument(script),
        );
        let steward = LocalModelSteward::new(steward()?, runner);

        let proposals = steward.propose(LocalModelStewardInput::new(
            created_at(),
            "return no proposals",
        ))?;

        assert!(proposals.is_empty());
        Ok(())
    }

    #[cfg(feature = "local-model")]
    #[test]
    fn local_executable_runner_maps_process_failure() -> Result<(), Box<dyn std::error::Error>> {
        let script = write_local_model_script(
            "continuitydb-local-model-fail",
            "cat >/dev/null\nprintf '%s\\n' 'model failed' >&2\nexit 7\n",
        )?;
        let runner = LocalExecutableRunner::new(
            LocalExecutableRunnerConfig::new("sh").with_argument(script),
        );
        let steward = LocalModelSteward::new(steward()?, runner);

        let result = steward.propose(LocalModelStewardInput::new(created_at(), "fail"));

        assert!(matches!(
            result,
            Err(StewardError::LocalModelExecutionFailed)
        ));
        Ok(())
    }

    #[cfg(feature = "local-model")]
    #[test]
    fn llama_cpp_runtime_profile_builds_deterministic_runner_config() {
        let profile = LlamaCppRuntimeProfile::new("llama-cli", "models/qwen2.5-0.5b.gguf")
            .with_context_size(8192)
            .with_temperature("0")
            .with_grammar_file("schemas/steward-proposal.gbnf");

        let config = profile.runner_config();

        assert_eq!(config.executable(), std::path::Path::new("llama-cli"));
        assert_eq!(
            config.command_arguments(),
            &[
                "--model".to_string(),
                "models/qwen2.5-0.5b.gguf".to_string(),
                "--ctx-size".to_string(),
                "8192".to_string(),
                "--temp".to_string(),
                "0".to_string(),
                "--grammar-file".to_string(),
                "schemas/steward-proposal.gbnf".to_string(),
                "--prompt".to_string(),
                "-".to_string(),
            ]
        );
    }

    #[cfg(feature = "local-model")]
    #[test]
    fn mistral_rs_runtime_profile_builds_deterministic_runner_config() {
        let profile = MistralRsRuntimeProfile::new("mistralrs-cli", "models/qwen2.5-0.5b.gguf")
            .with_context_size(4096)
            .with_temperature("0")
            .with_json_output();

        let config = profile.runner_config();

        assert_eq!(config.executable(), std::path::Path::new("mistralrs-cli"));
        assert_eq!(
            config.command_arguments(),
            &[
                "--model".to_string(),
                "models/qwen2.5-0.5b.gguf".to_string(),
                "--max-seq-len".to_string(),
                "4096".to_string(),
                "--temperature".to_string(),
                "0".to_string(),
                "--json-output".to_string(),
                "--prompt-stdin".to_string(),
            ]
        );
    }

    #[cfg(feature = "local-model")]
    #[test]
    fn runtime_profiles_accept_steward_response_contract_helpers() {
        let llama_config = LlamaCppRuntimeProfile::new("llama-cli", "models/qwen.gguf")
            .with_steward_response_grammar_file("schemas/steward-response.gbnf")
            .runner_config();

        assert!(llama_config
            .command_arguments()
            .windows(2)
            .any(|pair| pair == ["--grammar-file", "schemas/steward-response.gbnf"]));

        let mistral_config = MistralRsRuntimeProfile::new("mistralrs-server", "models/qwen.gguf")
            .with_steward_json_output()
            .runner_config();

        assert!(mistral_config
            .command_arguments()
            .iter()
            .any(|argument| argument == "--json-output"));
    }

    #[cfg(feature = "local-model")]
    #[test]
    fn local_model_benchmark_report_preserves_runtime_manifest(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let runner = LocalExecutableRunner::new(
            LocalExecutableRunnerConfig::new("llama-cli")
                .with_model_path("/models/qwen.gguf")
                .with_argument("--temp")
                .with_argument("0"),
        );
        let benchmark = LocalModelBenchmark::new(
            small_model_candidates()[0],
            runner,
            StewardEvaluationSuite::new(Vec::new()),
        );

        let report = benchmark.run(steward()?);

        assert_eq!(report.runtime().executable(), "llama-cli");
        assert_eq!(
            report.runtime().arguments(),
            &[
                "--model".to_string(),
                "/models/qwen.gguf".to_string(),
                "--temp".to_string(),
                "0".to_string(),
            ]
        );
        Ok(())
    }

    #[cfg(feature = "local-model")]
    #[test]
    fn local_model_benchmark_report_preserves_response_schema_version(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let benchmark = LocalModelBenchmark::new(
            small_model_candidates()[0],
            LocalExecutableRunner::new(LocalExecutableRunnerConfig::new("llama-cli")),
            StewardEvaluationSuite::new(Vec::new()),
        );

        let report = benchmark.run(steward()?);

        assert_eq!(
            report.response_schema_version(),
            LOCAL_MODEL_RESPONSE_SCHEMA_VERSION
        );
        Ok(())
    }

    #[cfg(feature = "local-model")]
    #[test]
    fn local_model_benchmark_baseline_preserves_runtime_manifest(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let runner = LocalExecutableRunner::new(
            LocalExecutableRunnerConfig::new("mistralrs-cli")
                .with_model_path("/models/qwen.gguf")
                .with_argument("--json-output"),
        );
        let benchmark = LocalModelBenchmark::new(
            small_model_candidates()[0],
            runner,
            StewardEvaluationSuite::new(Vec::new()),
        );
        let report = benchmark.run(steward()?);
        let expected_runtime = report.runtime().clone();

        let baseline = LocalModelBenchmarkBaseline::from_report(report, created_at());

        assert_eq!(baseline.runtime(), &expected_runtime);
        Ok(())
    }

    #[cfg(feature = "local-model")]
    #[test]
    fn local_model_benchmark_baseline_preserves_response_schema_version(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let benchmark = LocalModelBenchmark::new(
            small_model_candidates()[0],
            LocalExecutableRunner::new(LocalExecutableRunnerConfig::new("llama-cli")),
            StewardEvaluationSuite::new(Vec::new()),
        );
        let report = benchmark.run(steward()?);

        let baseline = LocalModelBenchmarkBaseline::from_report(report, created_at());

        assert_eq!(
            baseline.response_schema_version(),
            LOCAL_MODEL_RESPONSE_SCHEMA_VERSION
        );
        Ok(())
    }

    #[cfg(feature = "local-model")]
    #[test]
    fn local_model_benchmark_report_and_baseline_preserve_suite_fingerprint(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let suite = default_steward_evaluation_suite();
        let expected = suite.fingerprint();
        let benchmark = LocalModelBenchmark::new(
            small_model_candidates()[0],
            LocalExecutableRunner::new(LocalExecutableRunnerConfig::new("llama-cli")),
            suite,
        );

        let report = benchmark.run(steward()?);
        let baseline = LocalModelBenchmarkBaseline::from_report(report.clone(), created_at());

        assert_eq!(report.evaluation_suite_fingerprint(), expected);
        assert_eq!(baseline.evaluation_suite_fingerprint(), expected);
        Ok(())
    }

    #[cfg(feature = "local-model")]
    #[test]
    fn local_model_benchmark_report_and_baseline_preserve_contract_fingerprints(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let benchmark = LocalModelBenchmark::new(
            small_model_candidates()[0],
            LocalExecutableRunner::new(LocalExecutableRunnerConfig::new("llama-cli")),
            StewardEvaluationSuite::new(Vec::new()),
        );

        let report = benchmark.run(steward()?);
        let baseline = LocalModelBenchmarkBaseline::from_report(report.clone(), created_at());

        assert!(report.schema_fingerprint().starts_with("fnv1a64:"));
        assert!(report.grammar_fingerprint().starts_with("fnv1a64:"));
        assert_eq!(baseline.schema_fingerprint(), report.schema_fingerprint());
        assert_eq!(baseline.grammar_fingerprint(), report.grammar_fingerprint());
        Ok(())
    }

    #[cfg(feature = "local-model")]
    #[test]
    fn local_model_benchmark_report_and_baseline_preserve_prompt_fingerprint(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let benchmark = LocalModelBenchmark::new(
            small_model_candidates()[0],
            LocalExecutableRunner::new(LocalExecutableRunnerConfig::new("llama-cli")),
            default_steward_evaluation_suite(),
        );

        let report = benchmark.run(steward()?);
        let baseline = LocalModelBenchmarkBaseline::from_report(report.clone(), created_at());

        assert!(report.prompt_fingerprint().starts_with("fnv1a64:"));
        assert_eq!(baseline.prompt_fingerprint(), report.prompt_fingerprint());
        Ok(())
    }

    #[cfg(feature = "local-model")]
    #[test]
    fn local_model_benchmark_baseline_decodes_legacy_json_without_runtime_manifest(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let encoded = serde_json::json!({
            "candidate_model_id": "Qwen/Qwen2.5-0.5B-Instruct",
            "candidate_role": "default-feasibility",
            "evaluation": { "case_reports": [] },
            "recorded_at": created_at(),
        });

        let baseline: LocalModelBenchmarkBaseline = serde_json::from_value(encoded)?;

        assert_eq!(baseline.runtime().executable(), "");
        assert!(baseline.runtime().arguments().is_empty());
        assert_eq!(baseline.response_schema_version(), 0);
        Ok(())
    }

    #[cfg(feature = "local-model")]
    #[test]
    fn local_model_benchmark_baseline_decodes_legacy_json_without_suite_fingerprint(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let encoded = serde_json::json!({
            "candidate_model_id": "Qwen/Qwen2.5-0.5B-Instruct",
            "candidate_role": "default-feasibility",
            "response_schema_version": LOCAL_MODEL_RESPONSE_SCHEMA_VERSION,
            "runtime": { "executable": "sh", "arguments": [] },
            "evaluation": { "case_reports": [] },
            "recorded_at": created_at(),
        });

        let baseline: LocalModelBenchmarkBaseline = serde_json::from_value(encoded)?;

        assert_eq!(baseline.evaluation_suite_fingerprint(), "");
        Ok(())
    }

    #[cfg(feature = "local-model")]
    #[test]
    fn local_model_benchmark_baseline_decodes_legacy_json_without_contract_fingerprints(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let encoded = serde_json::json!({
            "candidate_model_id": "Qwen/Qwen2.5-0.5B-Instruct",
            "candidate_role": "default-feasibility",
            "response_schema_version": LOCAL_MODEL_RESPONSE_SCHEMA_VERSION,
            "evaluation_suite_fingerprint": default_steward_evaluation_suite().fingerprint(),
            "runtime": { "executable": "sh", "arguments": [] },
            "evaluation": { "case_reports": [] },
            "recorded_at": created_at(),
        });

        let baseline: LocalModelBenchmarkBaseline = serde_json::from_value(encoded)?;

        assert_eq!(baseline.schema_fingerprint(), "");
        assert_eq!(baseline.grammar_fingerprint(), "");
        Ok(())
    }

    #[cfg(feature = "local-model")]
    #[test]
    fn local_model_benchmark_baseline_decodes_legacy_json_without_prompt_fingerprint(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let encoded = serde_json::json!({
            "candidate_model_id": "Qwen/Qwen2.5-0.5B-Instruct",
            "candidate_role": "default-feasibility",
            "response_schema_version": LOCAL_MODEL_RESPONSE_SCHEMA_VERSION,
            "evaluation_suite_fingerprint": default_steward_evaluation_suite().fingerprint(),
            "schema_fingerprint": "fnv1a64:1111111111111111",
            "grammar_fingerprint": "fnv1a64:2222222222222222",
            "runtime": { "executable": "sh", "arguments": [] },
            "evaluation": { "case_reports": [] },
            "recorded_at": created_at(),
        });

        let baseline: LocalModelBenchmarkBaseline = serde_json::from_value(encoded)?;

        assert_eq!(baseline.prompt_fingerprint(), "");
        Ok(())
    }

    #[cfg(feature = "local-model")]
    #[test]
    fn local_model_benchmark_runs_executable_runner_suite() -> Result<(), Box<dyn std::error::Error>>
    {
        let cell_id = StateCellId::new();
        let response = serde_json::json!({
            "proposals": [{
                "action": {
                    "type": "mark_frontier",
                    "cell_id": cell_id,
                },
                "rationale": "The supplied evidence is stale.",
                "citations": ["test://frontier"]
            }]
        })
        .to_string();
        let script = write_local_model_script(
            "continuitydb-local-model-benchmark-ok",
            &format!("cat >/dev/null\nprintf '%s\\n' '{response}'\n"),
        )?;
        let runner = LocalExecutableRunner::new(
            LocalExecutableRunnerConfig::new("sh").with_argument(script),
        );
        let suite = StewardEvaluationSuite::new(vec![StewardEvaluationCase::new(
            "frontier",
            created_at(),
            "mark frontier",
        )
        .with_evidence("test://frontier", "Evidence is stale.")
        .expect_action(StewardAction::MarkFrontier { cell_id })
        .require_citation("test://frontier")]);
        let benchmark = LocalModelBenchmark::new(small_model_candidates()[0], runner, suite);

        let report = benchmark.run(steward()?);

        assert!(report.passed());
        assert_eq!(report.candidate().model_id(), "Qwen/Qwen2.5-0.5B-Instruct");
        assert_eq!(report.evaluation().case_reports().len(), 1);
        Ok(())
    }

    #[cfg(feature = "local-model")]
    #[test]
    fn local_model_benchmark_report_preserves_response_fingerprints(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let cell_id = StateCellId::new();
        let response = serde_json::json!({
            "proposals": [{
                "action": {
                    "type": "mark_frontier",
                    "cell_id": cell_id,
                },
                "rationale": "The supplied evidence is stale.",
                "citations": ["test://frontier"]
            }]
        })
        .to_string();
        let script = write_local_model_script(
            "continuitydb-local-model-response-fingerprints",
            &format!("cat >/dev/null\nprintf '%s\\n' '{response}'\n"),
        )?;
        let runner = LocalExecutableRunner::new(
            LocalExecutableRunnerConfig::new("sh").with_argument(script),
        );
        let suite = StewardEvaluationSuite::new(vec![StewardEvaluationCase::new(
            "frontier response fingerprint",
            created_at(),
            "mark frontier",
        )
        .with_evidence("test://frontier", "Evidence is stale.")
        .expect_action(StewardAction::MarkFrontier { cell_id })
        .require_citation("test://frontier")]);
        let benchmark = LocalModelBenchmark::new(small_model_candidates()[0], runner, suite);

        let report = benchmark.run(steward()?);
        let baseline = LocalModelBenchmarkBaseline::from_report(report.clone(), created_at());

        assert_eq!(report.response_fingerprints().len(), 1);
        assert_eq!(
            report.response_fingerprints()[0].case_name(),
            "frontier response fingerprint"
        );
        assert!(report.response_fingerprints()[0].captured());
        assert!(report.response_fingerprints()[0]
            .response_fingerprint()
            .is_some_and(|fingerprint| fingerprint.starts_with("fnv1a64:")));
        assert_eq!(
            report.response_fingerprints()[0].response_bytes(),
            format!("{response}\n").len()
        );
        assert_eq!(
            baseline.response_fingerprints(),
            report.response_fingerprints()
        );
        Ok(())
    }

    #[cfg(feature = "local-model")]
    #[test]
    fn local_model_benchmark_stability_passes_for_repeated_identical_outputs(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let cell_id = StateCellId::new();
        let response = serde_json::json!({
            "proposals": [{
                "action": {
                    "type": "mark_frontier",
                    "cell_id": cell_id,
                },
                "rationale": "The supplied evidence is stale.",
                "citations": ["test://frontier"]
            }]
        })
        .to_string();
        let script = write_local_model_script(
            "continuitydb-local-model-stability-ok",
            &format!("cat >/dev/null\nprintf '%s\\n' '{response}'\n"),
        )?;
        let runner = LocalExecutableRunner::new(
            LocalExecutableRunnerConfig::new("sh").with_argument(script),
        );
        let suite = StewardEvaluationSuite::new(vec![StewardEvaluationCase::new(
            "frontier",
            created_at(),
            "mark frontier",
        )
        .with_evidence("test://frontier", "Evidence is stale.")
        .expect_action(StewardAction::MarkFrontier { cell_id })
        .require_citation("test://frontier")]);
        let benchmark = LocalModelBenchmark::new(small_model_candidates()[0], runner, suite);

        let report = benchmark.run_stability(steward()?, 2);

        assert_eq!(report.trials(), 2);
        assert!(report.stable());
        assert_eq!(report.case_reports()[0].name(), "frontier");
        assert!(report.case_reports()[0].stable());
        assert!(report.case_reports()[0].changed_trials().is_empty());
        assert_eq!(report.case_reports()[0].proposal_fingerprints().len(), 2);
        assert_eq!(
            report.case_reports()[0].proposal_fingerprints()[0],
            report.case_reports()[0].proposal_fingerprints()[1]
        );
        Ok(())
    }

    #[cfg(feature = "local-model")]
    #[test]
    fn local_model_benchmark_stability_detects_drift_between_passing_outputs(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let cell_id = StateCellId::new();
        let count_path = std::env::temp_dir().join(format!(
            "continuitydb-local-model-stability-count-{:?}",
            ProposalId::new()
        ));
        let first_response = serde_json::json!({
            "proposals": [{
                "action": {
                    "type": "mark_frontier",
                    "cell_id": cell_id,
                },
                "rationale": "The supplied evidence is stale.",
                "citations": ["test://frontier"]
            }]
        })
        .to_string();
        let second_response = serde_json::json!({
            "proposals": [{
                "action": {
                    "type": "mark_frontier",
                    "cell_id": cell_id,
                },
                "rationale": "The supplied evidence remains stale.",
                "citations": ["test://frontier"]
            }]
        })
        .to_string();
        let script = write_local_model_script(
            "continuitydb-local-model-stability-drift",
            &format!(
                "cat >/dev/null\nif [ -f '{count}' ]; then printf '%s\\n' '{second}'; else touch '{count}'; printf '%s\\n' '{first}'; fi\n",
                count = count_path.display(),
                first = first_response,
                second = second_response,
            ),
        )?;
        let runner = LocalExecutableRunner::new(
            LocalExecutableRunnerConfig::new("sh").with_argument(script),
        );
        let suite = StewardEvaluationSuite::new(vec![StewardEvaluationCase::new(
            "frontier",
            created_at(),
            "mark frontier",
        )
        .with_evidence("test://frontier", "Evidence is stale.")
        .expect_action(StewardAction::MarkFrontier { cell_id })
        .require_citation("test://frontier")]);
        let benchmark = LocalModelBenchmark::new(small_model_candidates()[0], runner, suite);

        let report = benchmark.run_stability(steward()?, 2);

        assert_eq!(report.trials(), 2);
        assert!(!report.stable());
        assert_eq!(report.case_reports()[0].name(), "frontier");
        assert!(!report.case_reports()[0].stable());
        assert_eq!(report.case_reports()[0].changed_trials(), &[2]);
        assert_ne!(
            report.case_reports()[0].proposal_fingerprints()[0],
            report.case_reports()[0].proposal_fingerprints()[1]
        );

        let _ = fs::remove_file(count_path);
        Ok(())
    }

    #[cfg(feature = "local-model")]
    #[test]
    fn local_model_benchmark_preserves_failure_report() -> Result<(), Box<dyn std::error::Error>> {
        let cell_id = StateCellId::new();
        let response = serde_json::json!({
            "proposals": [{
                "action": {
                    "type": "mark_frontier",
                    "cell_id": cell_id,
                },
                "rationale": "This was verified in production.",
                "citations": ["test://other"]
            }]
        })
        .to_string();
        let script = write_local_model_script(
            "continuitydb-local-model-benchmark-fail",
            &format!("cat >/dev/null\nprintf '%s\\n' '{response}'\n"),
        )?;
        let runner = LocalExecutableRunner::new(
            LocalExecutableRunnerConfig::new("sh").with_argument(script),
        );
        let suite = StewardEvaluationSuite::new(vec![StewardEvaluationCase::new(
            "frontier failure",
            created_at(),
            "mark frontier",
        )
        .with_evidence("test://frontier", "Evidence is stale.")
        .expect_action(StewardAction::MarkFrontier { cell_id })
        .require_citation("test://frontier")
        .forbid_rationale_term("verified in production")]);
        let benchmark = LocalModelBenchmark::new(small_model_candidates()[0], runner, suite);

        let report = benchmark.run(steward()?);

        assert!(!report.passed());
        assert_eq!(
            report.evaluation().case_reports()[0].failures(),
            &[
                StewardEvaluationFailure::MissingCitation {
                    locator: "test://frontier".to_string(),
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
    fn local_model_benchmark_baseline_preserves_report_metadata(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let cell_id = StateCellId::new();
        let response = serde_json::json!({
            "proposals": [{
                "action": {
                    "type": "mark_frontier",
                    "cell_id": cell_id,
                },
                "rationale": "The supplied evidence is stale.",
                "citations": ["test://frontier"]
            }]
        })
        .to_string();
        let script = write_local_model_script(
            "continuitydb-local-model-baseline-ok",
            &format!("cat >/dev/null\nprintf '%s\\n' '{response}'\n"),
        )?;
        let runner = LocalExecutableRunner::new(
            LocalExecutableRunnerConfig::new("sh").with_argument(script),
        );
        let suite = StewardEvaluationSuite::new(vec![StewardEvaluationCase::new(
            "frontier baseline",
            created_at(),
            "mark frontier",
        )
        .with_evidence("test://frontier", "Evidence is stale.")
        .expect_action(StewardAction::MarkFrontier { cell_id })
        .require_citation("test://frontier")]);
        let benchmark = LocalModelBenchmark::new(small_model_candidates()[0], runner, suite);

        let baseline =
            LocalModelBenchmarkBaseline::from_report(benchmark.run(steward()?), created_at());

        assert!(baseline.passed());
        assert_eq!(baseline.candidate_model_id(), "Qwen/Qwen2.5-0.5B-Instruct");
        assert_eq!(baseline.candidate_role(), "default-feasibility");
        assert_eq!(baseline.recorded_at(), created_at());
        assert_eq!(baseline.evaluation().case_reports().len(), 1);
        Ok(())
    }

    #[cfg(feature = "local-model")]
    #[test]
    fn local_model_benchmark_records_baseline_in_store() -> Result<(), Box<dyn std::error::Error>> {
        let cell_id = StateCellId::new();
        let response = serde_json::json!({
            "proposals": [{
                "action": {
                    "type": "mark_frontier",
                    "cell_id": cell_id,
                },
                "rationale": "The supplied evidence is stale.",
                "citations": ["test://frontier"]
            }]
        })
        .to_string();
        let script = write_local_model_script(
            "continuitydb-local-model-baseline-record",
            &format!("cat >/dev/null\nprintf '%s\\n' '{response}'\n"),
        )?;
        let runner = LocalExecutableRunner::new(
            LocalExecutableRunnerConfig::new("sh").with_argument(script),
        );
        let suite = StewardEvaluationSuite::new(vec![StewardEvaluationCase::new(
            "frontier baseline record",
            created_at(),
            "mark frontier",
        )
        .with_evidence("test://frontier", "Evidence is stale.")
        .expect_action(StewardAction::MarkFrontier { cell_id })
        .require_citation("test://frontier")]);
        let benchmark = LocalModelBenchmark::new(small_model_candidates()[0], runner, suite);
        let mut store = MemoryLocalModelBenchmarkBaselineStore::default();

        let baseline = record_local_model_benchmark_baseline(
            &benchmark,
            steward()?,
            created_at(),
            &mut store,
        )?;

        assert!(baseline.passed());
        assert_eq!(baseline.candidate_model_id(), "Qwen/Qwen2.5-0.5B-Instruct");
        assert_eq!(baseline.candidate_role(), "default-feasibility");
        assert_eq!(store.list_baselines()?, vec![baseline]);
        Ok(())
    }

    #[cfg(feature = "local-model")]
    #[test]
    fn local_model_baseline_regression_detects_pass_count_drop(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let cell_id = StateCellId::new();
        let previous = local_model_baseline_for_response(
            cell_id,
            serde_json::json!({
                "proposals": [{
                    "action": {
                        "type": "mark_frontier",
                        "cell_id": cell_id,
                    },
                    "rationale": "The supplied evidence is stale.",
                    "citations": ["test://frontier"]
                }]
            })
            .to_string(),
        )?;
        let current = local_model_baseline_for_response(
            cell_id,
            serde_json::json!({
                "proposals": [{
                    "action": {
                        "type": "mark_frontier",
                        "cell_id": cell_id,
                    },
                    "rationale": "The supplied evidence is stale.",
                    "citations": ["test://other"]
                }]
            })
            .to_string(),
        )?;

        let regression = LocalModelBenchmarkRegression::compare(&previous, &current);

        assert!(regression.regressed());
        assert_eq!(regression.previous_passed_cases(), 1);
        assert_eq!(regression.current_passed_cases(), 0);
        assert_eq!(regression.pass_count_delta(), -1);
        Ok(())
    }

    #[cfg(feature = "local-model")]
    #[test]
    fn local_model_baseline_regression_allows_equal_quality(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let cell_id = StateCellId::new();
        let response = serde_json::json!({
            "proposals": [{
                "action": {
                    "type": "mark_frontier",
                    "cell_id": cell_id,
                },
                "rationale": "The supplied evidence is stale.",
                "citations": ["test://frontier"]
            }]
        })
        .to_string();
        let previous = local_model_baseline_for_response(cell_id, response.clone())?;
        let current = local_model_baseline_for_response(cell_id, response)?;

        let regression = LocalModelBenchmarkRegression::compare(&previous, &current);

        assert!(!regression.regressed());
        assert_eq!(regression.pass_count_delta(), 0);
        Ok(())
    }

    #[cfg(feature = "local-model")]
    #[test]
    fn latest_local_model_baseline_returns_newest_matching_candidate(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let old_qwen = local_model_empty_baseline(
            small_model_candidates()[0],
            Utc.with_ymd_and_hms(2026, 5, 20, 1, 0, 0)
                .single()
                .unwrap_or_else(Utc::now),
        )?;
        let other_candidate = local_model_empty_baseline(
            small_model_candidates()[1],
            Utc.with_ymd_and_hms(2026, 5, 20, 3, 0, 0)
                .single()
                .unwrap_or_else(Utc::now),
        )?;
        let new_qwen = local_model_empty_baseline(
            small_model_candidates()[0],
            Utc.with_ymd_and_hms(2026, 5, 20, 2, 0, 0)
                .single()
                .unwrap_or_else(Utc::now),
        )?;
        let mut store = MemoryLocalModelBenchmarkBaselineStore::default();
        store.append_baseline(old_qwen)?;
        store.append_baseline(other_candidate)?;
        store.append_baseline(new_qwen.clone())?;

        let latest = latest_local_model_benchmark_baseline(&store, small_model_candidates()[0])?;

        assert_eq!(latest, Some(new_qwen));
        Ok(())
    }

    #[cfg(feature = "local-model")]
    #[test]
    fn latest_local_model_baseline_returns_none_without_candidate_match(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut store = MemoryLocalModelBenchmarkBaselineStore::default();
        store.append_baseline(local_model_empty_baseline(
            small_model_candidates()[1],
            created_at(),
        )?)?;

        let latest = latest_local_model_benchmark_baseline(&store, small_model_candidates()[0])?;

        assert_eq!(latest, None);
        Ok(())
    }

    #[cfg(feature = "local-model")]
    #[test]
    fn latest_compatible_local_model_baseline_matches_runtime_and_schema(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let old_compatible = local_model_empty_baseline(
            small_model_candidates()[0],
            Utc.with_ymd_and_hms(2026, 5, 20, 1, 0, 0)
                .single()
                .unwrap_or_else(Utc::now),
        )?;
        let incompatible_runtime = local_model_runtime_baseline(
            small_model_candidates()[0],
            Utc.with_ymd_and_hms(2026, 5, 20, 3, 0, 0)
                .single()
                .unwrap_or_else(Utc::now),
            "--different-runtime",
        )?;
        let new_compatible = local_model_empty_baseline(
            small_model_candidates()[0],
            Utc.with_ymd_and_hms(2026, 5, 20, 2, 0, 0)
                .single()
                .unwrap_or_else(Utc::now),
        )?;
        let current = local_model_empty_baseline(
            small_model_candidates()[0],
            Utc.with_ymd_and_hms(2026, 5, 20, 4, 0, 0)
                .single()
                .unwrap_or_else(Utc::now),
        )?;
        let mut store = MemoryLocalModelBenchmarkBaselineStore::default();
        store.append_baseline(old_compatible)?;
        store.append_baseline(incompatible_runtime)?;
        store.append_baseline(new_compatible.clone())?;

        let latest = latest_compatible_local_model_benchmark_baseline(&store, &current)?;

        assert_eq!(latest, Some(new_compatible));
        Ok(())
    }

    #[cfg(feature = "local-model")]
    #[test]
    fn latest_compatible_local_model_baseline_returns_none_without_compatible_runtime(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut store = MemoryLocalModelBenchmarkBaselineStore::default();
        store.append_baseline(local_model_runtime_baseline(
            small_model_candidates()[0],
            created_at(),
            "--different-runtime",
        )?)?;
        let current = local_model_empty_baseline(small_model_candidates()[0], created_at())?;

        let latest = latest_compatible_local_model_benchmark_baseline(&store, &current)?;

        assert_eq!(latest, None);
        Ok(())
    }

    #[cfg(feature = "local-model")]
    #[test]
    fn latest_compatible_local_model_baseline_requires_suite_fingerprint(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let old_compatible = local_model_empty_baseline(
            small_model_candidates()[0],
            Utc.with_ymd_and_hms(2026, 5, 20, 1, 0, 0)
                .single()
                .unwrap_or_else(Utc::now),
        )?;
        let incompatible_suite = local_model_suite_baseline(
            small_model_candidates()[0],
            Utc.with_ymd_and_hms(2026, 5, 20, 3, 0, 0)
                .single()
                .unwrap_or_else(Utc::now),
            StewardEvaluationSuite::new(vec![StewardEvaluationCase::new(
                "different suite",
                created_at(),
                "different task",
            )]),
        )?;
        let current = local_model_empty_baseline(
            small_model_candidates()[0],
            Utc.with_ymd_and_hms(2026, 5, 20, 4, 0, 0)
                .single()
                .unwrap_or_else(Utc::now),
        )?;
        let mut store = MemoryLocalModelBenchmarkBaselineStore::default();
        store.append_baseline(old_compatible.clone())?;
        store.append_baseline(incompatible_suite)?;

        let latest = latest_compatible_local_model_benchmark_baseline(&store, &current)?;

        assert_eq!(latest, Some(old_compatible));
        Ok(())
    }

    #[cfg(feature = "local-model")]
    #[test]
    fn latest_compatible_local_model_baseline_requires_contract_fingerprints(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let old_compatible = local_model_empty_baseline(
            small_model_candidates()[0],
            Utc.with_ymd_and_hms(2026, 5, 20, 1, 0, 0)
                .single()
                .unwrap_or_else(Utc::now),
        )?;
        let mut incompatible_json = serde_json::to_value(local_model_empty_baseline(
            small_model_candidates()[0],
            Utc.with_ymd_and_hms(2026, 5, 20, 2, 0, 0)
                .single()
                .unwrap_or_else(Utc::now),
        )?)?;
        incompatible_json["schema_fingerprint"] = serde_json::json!("fnv1a64:0000000000000000");
        let incompatible_contract: LocalModelBenchmarkBaseline =
            serde_json::from_value(incompatible_json)?;
        let current = local_model_empty_baseline(
            small_model_candidates()[0],
            Utc.with_ymd_and_hms(2026, 5, 20, 3, 0, 0)
                .single()
                .unwrap_or_else(Utc::now),
        )?;
        let mut store = MemoryLocalModelBenchmarkBaselineStore::default();
        store.append_baseline(old_compatible.clone())?;
        store.append_baseline(incompatible_contract)?;

        let latest = latest_compatible_local_model_benchmark_baseline(&store, &current)?;

        assert_eq!(latest, Some(old_compatible));
        Ok(())
    }

    #[cfg(feature = "local-model")]
    #[test]
    fn latest_compatible_local_model_baseline_requires_prompt_fingerprint(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let old_compatible = local_model_empty_baseline(
            small_model_candidates()[0],
            Utc.with_ymd_and_hms(2026, 5, 20, 1, 0, 0)
                .single()
                .unwrap_or_else(Utc::now),
        )?;
        let mut incompatible_json = serde_json::to_value(local_model_empty_baseline(
            small_model_candidates()[0],
            Utc.with_ymd_and_hms(2026, 5, 20, 2, 0, 0)
                .single()
                .unwrap_or_else(Utc::now),
        )?)?;
        incompatible_json["prompt_fingerprint"] = serde_json::json!("fnv1a64:0000000000000000");
        let incompatible_prompt: LocalModelBenchmarkBaseline =
            serde_json::from_value(incompatible_json)?;
        let current = local_model_empty_baseline(
            small_model_candidates()[0],
            Utc.with_ymd_and_hms(2026, 5, 20, 3, 0, 0)
                .single()
                .unwrap_or_else(Utc::now),
        )?;
        let mut store = MemoryLocalModelBenchmarkBaselineStore::default();
        store.append_baseline(old_compatible.clone())?;
        store.append_baseline(incompatible_prompt)?;

        let latest = latest_compatible_local_model_benchmark_baseline(&store, &current)?;

        assert_eq!(latest, Some(old_compatible));
        Ok(())
    }

    #[cfg(feature = "local-model")]
    #[test]
    fn local_model_benchmark_baseline_exposes_evaluation_summary(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let baseline = local_model_empty_baseline(small_model_candidates()[0], created_at())?;

        let summary = baseline.evaluation_summary();

        assert_eq!(summary.total_cases(), 0);
        assert_eq!(summary.passed_cases(), 0);
        assert_eq!(summary.failed_cases(), 0);
        assert_eq!(summary.pass_rate(), 1.0);
        assert!(summary.passed());
        Ok(())
    }

    #[cfg(feature = "local-model")]
    #[test]
    fn local_model_baseline_gate_records_without_previous_regression(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let cell_id = StateCellId::new();
        let benchmark = local_model_benchmark_for_response(
            cell_id,
            serde_json::json!({
                "proposals": [{
                    "action": {
                        "type": "mark_frontier",
                        "cell_id": cell_id,
                    },
                    "rationale": "The supplied evidence is stale.",
                    "citations": ["test://frontier"]
                }]
            })
            .to_string(),
        )?;
        let mut store = MemoryLocalModelBenchmarkBaselineStore::default();

        let report: LocalModelBenchmarkGateReport =
            record_local_model_benchmark_baseline_with_regression(
                &benchmark,
                steward()?,
                created_at(),
                &mut store,
            )?;

        assert!(report.current_baseline().passed());
        assert_eq!(report.regression(), None);
        assert!(!report.regressed());
        assert_eq!(
            store.list_baselines()?,
            vec![report.current_baseline().clone()]
        );
        Ok(())
    }

    #[cfg(feature = "local-model")]
    #[test]
    fn local_model_baseline_gate_reports_regression_against_latest_previous(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let cell_id = StateCellId::new();
        let response_path = temp_local_model_baseline_store_path(
            "continuitydb-local-model-compatible-regression-response",
        );
        fs::write(
            &response_path,
            serde_json::json!({
                "proposals": [{
                    "action": {
                        "type": "mark_frontier",
                        "cell_id": cell_id,
                    },
                    "rationale": "The supplied evidence is stale.",
                    "citations": ["test://frontier"]
                }]
            })
            .to_string(),
        )?;
        let benchmark = local_model_file_backed_benchmark(
            cell_id,
            "continuitydb-local-model-compatible-regression",
            &response_path,
        )?;
        let previous = LocalModelBenchmarkBaseline::from_report(
            benchmark.clone().run(steward()?),
            created_at(),
        );
        fs::write(
            &response_path,
            serde_json::json!({
                "proposals": [{
                    "action": {
                        "type": "mark_frontier",
                        "cell_id": cell_id,
                    },
                    "rationale": "The supplied evidence is stale.",
                    "citations": ["test://other"]
                }]
            })
            .to_string(),
        )?;
        let mut store = MemoryLocalModelBenchmarkBaselineStore::default();
        store.append_baseline(previous)?;

        let report: LocalModelBenchmarkGateReport =
            record_local_model_benchmark_baseline_with_regression(
                &benchmark,
                steward()?,
                created_at(),
                &mut store,
            )?;

        let Some(regression) = report.regression() else {
            return Err("previous baseline missing".into());
        };
        assert!(regression.regressed());
        assert_eq!(regression.previous_passed_cases(), 1);
        assert_eq!(regression.current_passed_cases(), 0);
        assert!(report.regressed());
        assert_eq!(store.list_baselines()?.len(), 2);
        fs::remove_file(response_path)?;
        Ok(())
    }

    #[cfg(feature = "local-model")]
    #[test]
    fn local_model_baseline_gate_skips_incompatible_runtime_baseline(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let cell_id = StateCellId::new();
        let previous = local_model_baseline_for_response(
            cell_id,
            serde_json::json!({
                "proposals": [{
                    "action": {
                        "type": "mark_frontier",
                        "cell_id": cell_id,
                    },
                    "rationale": "The supplied evidence is stale.",
                    "citations": ["test://frontier"]
                }]
            })
            .to_string(),
        )?;
        let benchmark = local_model_benchmark_for_response(
            cell_id,
            serde_json::json!({
                "proposals": [{
                    "action": {
                        "type": "mark_frontier",
                        "cell_id": cell_id,
                    },
                    "rationale": "The supplied evidence is stale.",
                    "citations": ["test://other"]
                }]
            })
            .to_string(),
        )?;
        let mut store = MemoryLocalModelBenchmarkBaselineStore::default();
        store.append_baseline(previous)?;

        let report = record_local_model_benchmark_baseline_with_regression(
            &benchmark,
            steward()?,
            created_at(),
            &mut store,
        )?;

        assert_eq!(report.regression(), None);
        assert!(!report.regressed());
        assert_eq!(store.list_baselines()?.len(), 2);
        Ok(())
    }

    #[cfg(feature = "local-model")]
    #[test]
    fn memory_local_model_benchmark_baseline_store_lists_in_order(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let first = LocalModelBenchmarkBaseline::from_report(
            LocalModelBenchmark::new(
                small_model_candidates()[0],
                LocalExecutableRunner::new(LocalExecutableRunnerConfig::new("sh")),
                StewardEvaluationSuite::new(Vec::new()),
            )
            .run(steward()?),
            created_at(),
        );
        let second = LocalModelBenchmarkBaseline::from_report(
            LocalModelBenchmark::new(
                small_model_candidates()[1],
                LocalExecutableRunner::new(LocalExecutableRunnerConfig::new("sh")),
                StewardEvaluationSuite::new(Vec::new()),
            )
            .run(steward()?),
            created_at(),
        );
        let mut store = MemoryLocalModelBenchmarkBaselineStore::default();

        store.append_baseline(first.clone())?;
        store.append_baseline(second.clone())?;

        let baselines = store.list_baselines()?;
        assert_eq!(baselines, vec![first, second]);
        Ok(())
    }

    #[cfg(feature = "local-model")]
    #[test]
    fn file_local_model_benchmark_baseline_store_persists_across_reopen(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_local_model_baseline_store_path("continuitydb-local-model-baselines");
        let baseline = LocalModelBenchmarkBaseline::from_report(
            LocalModelBenchmark::new(
                small_model_candidates()[0],
                LocalExecutableRunner::new(LocalExecutableRunnerConfig::new("sh")),
                StewardEvaluationSuite::new(Vec::new()),
            )
            .run(steward()?),
            created_at(),
        );

        {
            let mut store = FileLocalModelBenchmarkBaselineStore::open(&path)?;
            store.append_baseline(baseline.clone())?;
        }

        let reopened = FileLocalModelBenchmarkBaselineStore::open(&path)?;
        assert_eq!(reopened.list_baselines()?, vec![baseline]);
        fs::remove_file(path)?;
        Ok(())
    }

    #[cfg(feature = "local-model")]
    #[test]
    fn file_local_model_benchmark_baseline_store_rejects_invalid_jsonl(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path =
            temp_local_model_baseline_store_path("continuitydb-local-model-baselines-invalid");
        fs::write(&path, "{not valid json}\n")?;
        let store = FileLocalModelBenchmarkBaselineStore::open(&path)?;

        let result = store.list_baselines();

        assert!(matches!(
            result,
            Err(StewardError::LocalModelBenchmarkBaselineStoreCorrupt)
        ));
        fs::remove_file(path)?;
        Ok(())
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
        assert_eq!(
            proposals[0].action(),
            &StewardAction::RequestVerification {
                cell_id: Some(cell_id),
                request: "Refresh stale evidence for frontier cell.".to_string(),
            }
        );
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
            StewardAction::MarkFrontier { cell_id: actual } if *actual == cell_id
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

    #[test]
    fn frontier_subscription_rejects_empty_inputs() {
        let no_signals = FrontierSubscription::new(
            FrontierSubscriptionId::new(),
            StateCellId::new(),
            Vec::new(),
            "test://subscription",
            created_at(),
        );
        let no_citation = FrontierSubscription::new(
            FrontierSubscriptionId::new(),
            StateCellId::new(),
            vec![FrontierWatchSignal::StaleEvidence],
            " ",
            created_at(),
        );

        assert!(matches!(
            no_signals,
            Err(StewardError::EmptyFrontierSubscription)
        ));
        assert!(matches!(
            no_citation,
            Err(StewardError::EmptyFrontierSubscription)
        ));
    }

    #[test]
    fn frontier_subscription_matches_same_cell_and_signal() -> Result<(), Box<dyn std::error::Error>>
    {
        let cell_id = StateCellId::new();
        let subscription = FrontierSubscription::new(
            FrontierSubscriptionId::new(),
            cell_id,
            vec![FrontierWatchSignal::StaleEvidence],
            "test://subscription",
            created_at(),
        )?;

        let matched = FrontierWatchEvent::new(
            cell_id,
            FrontierWatchSignal::StaleEvidence,
            "test://stale",
            created_at(),
        );
        let wrong_signal = FrontierWatchEvent::new(
            cell_id,
            FrontierWatchSignal::HighImpactUncertainty,
            "test://uncertain",
            created_at(),
        );
        let wrong_cell = FrontierWatchEvent::new(
            StateCellId::new(),
            FrontierWatchSignal::StaleEvidence,
            "test://stale",
            created_at(),
        );

        assert!(subscription.matches_event(&matched));
        assert!(!subscription.matches_event(&wrong_signal));
        assert!(!subscription.matches_event(&wrong_cell));
        Ok(())
    }

    #[test]
    fn memory_frontier_subscription_store_lists_and_gets_by_id(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let first = FrontierSubscription::new(
            FrontierSubscriptionId::new(),
            StateCellId::new(),
            vec![FrontierWatchSignal::StaleEvidence],
            "test://first",
            created_at(),
        )?;
        let second = FrontierSubscription::new(
            FrontierSubscriptionId::new(),
            StateCellId::new(),
            vec![FrontierWatchSignal::HighImpactUncertainty],
            "test://second",
            created_at(),
        )?;
        let mut store = MemoryFrontierSubscriptionStore::default();

        store.append_subscription(first.clone())?;
        store.append_subscription(second.clone())?;

        let subscriptions = store.list_subscriptions()?;
        assert_eq!(subscriptions.len(), 2);
        assert_eq!(subscriptions[0].id(), first.id());
        assert_eq!(subscriptions[1].id(), second.id());
        assert_eq!(
            store
                .get_subscription(second.id())?
                .map(|subscription| subscription.id()),
            Some(second.id())
        );
        Ok(())
    }

    #[test]
    fn file_frontier_subscription_store_persists_across_reopen(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_frontier_subscription_store_path("continuitydb-frontier-subscriptions");
        let subscription = FrontierSubscription::new(
            FrontierSubscriptionId::new(),
            StateCellId::new(),
            vec![
                FrontierWatchSignal::StaleEvidence,
                FrontierWatchSignal::HighImpactUncertainty,
            ],
            "test://subscription",
            created_at(),
        )?;

        {
            let mut store = FileFrontierSubscriptionStore::open(&path)?;
            store.append_subscription(subscription.clone())?;
        }

        let reopened = FileFrontierSubscriptionStore::open(&path)?;
        let subscriptions = reopened.list_subscriptions()?;
        assert_eq!(subscriptions.len(), 1);
        assert_eq!(subscriptions[0], subscription);
        assert_eq!(
            reopened
                .get_subscription(subscription.id())?
                .map(|stored| stored.id()),
            Some(subscription.id())
        );
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_frontier_subscription_store_rejects_invalid_jsonl(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_frontier_subscription_store_path("continuitydb-frontier-subscriptions-bad");
        fs::write(&path, "{not valid json}\n")?;
        let store = FileFrontierSubscriptionStore::open(&path)?;

        let result = store.list_subscriptions();

        assert!(matches!(
            result,
            Err(StewardError::FrontierSubscriptionStoreCorrupt)
        ));
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn frontier_subscription_runner_emits_matching_verification_proposal(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let cell_id = StateCellId::new();
        let subscription = FrontierSubscription::new(
            FrontierSubscriptionId::new(),
            cell_id,
            vec![FrontierWatchSignal::StaleEvidence],
            "test://subscription",
            created_at(),
        )?;
        let mut store = MemoryFrontierSubscriptionStore::default();
        store.append_subscription(subscription)?;
        let runner = FrontierSubscriptionRunner::new(FrontierSteward::new(steward()?), store);

        let proposals = runner.propose_subscribed(
            vec![FrontierWatchEvent::new(
                cell_id,
                FrontierWatchSignal::StaleEvidence,
                "test://stale",
                created_at(),
            )],
            created_at(),
        )?;

        assert_eq!(proposals.len(), 1);
        assert_eq!(
            proposals[0].action(),
            &StewardAction::RequestVerification {
                cell_id: Some(cell_id),
                request: "Refresh stale evidence for frontier cell.".to_string(),
            }
        );
        assert_eq!(proposals[0].citations(), &["test://stale".to_string()]);
        Ok(())
    }

    #[test]
    fn frontier_subscription_runner_suppresses_unsubscribed_events(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let cell_id = StateCellId::new();
        let subscription = FrontierSubscription::new(
            FrontierSubscriptionId::new(),
            cell_id,
            vec![FrontierWatchSignal::StaleEvidence],
            "test://subscription",
            created_at(),
        )?;
        let mut store = MemoryFrontierSubscriptionStore::default();
        store.append_subscription(subscription)?;
        let runner = FrontierSubscriptionRunner::new(FrontierSteward::new(steward()?), store);

        let proposals = runner.propose_subscribed(
            vec![
                FrontierWatchEvent::new(
                    cell_id,
                    FrontierWatchSignal::HighImpactUncertainty,
                    "test://wrong-signal",
                    created_at(),
                ),
                FrontierWatchEvent::new(
                    StateCellId::new(),
                    FrontierWatchSignal::StaleEvidence,
                    "test://wrong-cell",
                    created_at(),
                ),
            ],
            created_at(),
        )?;

        assert!(proposals.is_empty());
        Ok(())
    }

    #[test]
    fn frontier_subscription_runner_deduplicates_multiple_matching_subscriptions(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let cell_id = StateCellId::new();
        let first = FrontierSubscription::new(
            FrontierSubscriptionId::new(),
            cell_id,
            vec![FrontierWatchSignal::HighImpactUncertainty],
            "test://first",
            created_at(),
        )?;
        let second = FrontierSubscription::new(
            FrontierSubscriptionId::new(),
            cell_id,
            vec![FrontierWatchSignal::HighImpactUncertainty],
            "test://second",
            created_at(),
        )?;
        let mut store = MemoryFrontierSubscriptionStore::default();
        store.append_subscription(first)?;
        store.append_subscription(second)?;
        let runner = FrontierSubscriptionRunner::new(FrontierSteward::new(steward()?), store);

        let proposals = runner.propose_subscribed(
            vec![FrontierWatchEvent::new(
                cell_id,
                FrontierWatchSignal::HighImpactUncertainty,
                "test://uncertain",
                created_at(),
            )],
            created_at(),
        )?;

        assert_eq!(proposals.len(), 1);
        assert_eq!(
            proposals[0].action(),
            &StewardAction::MarkFrontier { cell_id }
        );
        Ok(())
    }

    #[test]
    fn stored_proposal_ledger_records_in_store_order() -> Result<(), Box<dyn std::error::Error>> {
        let first = valid_link_revision()?;
        let second = StewardProposal::new(
            ProposalId::new(),
            steward()?,
            StewardAction::RequestVerification {
                cell_id: Some(StateCellId::new()),
                request: "Refresh stale evidence.".to_string(),
            },
            "The watched cell needs evidence refresh.",
            vec!["test://stale".to_string()],
            created_at(),
        )?;
        let policy = ProposalPolicy::strict();
        let mut ledger = StoredProposalLedger::new(MemoryProposalStore::default());

        ledger.record(first.clone(), policy.evaluate(&first, created_at()))?;
        ledger.record(second.clone(), policy.evaluate(&second, created_at()))?;

        let records = ledger.records()?;
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].proposal().id(), first.id());
        assert_eq!(records[1].proposal().id(), second.id());
        Ok(())
    }

    #[test]
    fn stored_proposal_ledger_gets_record_by_id() -> Result<(), Box<dyn std::error::Error>> {
        let proposal = valid_link_revision()?;
        let decision = ProposalPolicy::strict().evaluate(&proposal, created_at());
        let mut ledger = StoredProposalLedger::new(MemoryProposalStore::default());

        ledger.record(proposal.clone(), decision)?;

        let record = ledger.record_by_id(proposal.id())?;
        assert_eq!(
            record.map(|record| record.proposal().id()),
            Some(proposal.id())
        );
        Ok(())
    }

    #[test]
    fn stored_proposal_ledger_rejects_mismatch_without_mutating_store(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let proposal = valid_link_revision()?;
        let mismatched_decision = ProposalDecision::new(
            ProposalId::new(),
            ProposalOutcome::Accepted,
            vec!["policy:structurally-valid".to_string()],
            created_at(),
        );
        let mut ledger = StoredProposalLedger::new(MemoryProposalStore::default());

        let result = ledger.record(proposal, mismatched_decision);

        assert!(matches!(result, Err(StewardError::MismatchedDecision)));
        assert!(ledger.records()?.is_empty());
        Ok(())
    }

    #[test]
    fn kernel_proposal_store_records_in_kernel_order() -> Result<(), Box<dyn std::error::Error>> {
        let first = valid_link_revision()?;
        let second = StewardProposal::new(
            ProposalId::new(),
            steward()?,
            StewardAction::RequestVerification {
                cell_id: Some(StateCellId::new()),
                request: "Refresh stale evidence.".to_string(),
            },
            "The watched cell needs evidence refresh.",
            vec!["test://stale".to_string()],
            created_at(),
        )?;
        let policy = ProposalPolicy::strict();
        let store = KernelProposalStore::new(continuitydb_memory::MemoryKernel::default());
        let mut ledger = StoredProposalLedger::new(store);

        ledger.record(first.clone(), policy.evaluate(&first, created_at()))?;
        ledger.record(second.clone(), policy.evaluate(&second, created_at()))?;

        let records = ledger.records()?;
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].proposal().id(), first.id());
        assert_eq!(records[1].proposal().id(), second.id());
        Ok(())
    }

    #[test]
    fn kernel_proposal_store_gets_record_by_id() -> Result<(), Box<dyn std::error::Error>> {
        let first = valid_link_revision()?;
        let second = StewardProposal::new(
            ProposalId::new(),
            steward()?,
            StewardAction::MarkFrontier {
                cell_id: StateCellId::new(),
            },
            "The watched cell needs frontier monitoring.",
            vec!["test://frontier".to_string()],
            created_at(),
        )?;
        let policy = ProposalPolicy::strict();
        let store = KernelProposalStore::new(continuitydb_memory::MemoryKernel::default());
        let mut ledger = StoredProposalLedger::new(store);

        ledger.record(first.clone(), policy.evaluate(&first, created_at()))?;
        ledger.record(second.clone(), policy.evaluate(&second, created_at()))?;

        let record = ledger.record_by_id(second.id())?;
        assert_eq!(
            record.map(|record| record.proposal().id()),
            Some(second.id())
        );
        Ok(())
    }

    #[test]
    fn kernel_proposal_store_preserves_backing_kernel() -> Result<(), Box<dyn std::error::Error>> {
        let proposal = valid_link_revision()?;
        let policy = ProposalPolicy::strict();
        let store = KernelProposalStore::new(continuitydb_memory::MemoryKernel::default());
        let mut ledger = StoredProposalLedger::new(store);

        ledger.record(proposal.clone(), policy.evaluate(&proposal, created_at()))?;

        let store = ledger.into_store();
        let reopened = StoredProposalLedger::new(KernelProposalStore::new(store.into_kernel()));
        assert_eq!(reopened.records()?.len(), 1);
        Ok(())
    }

    #[test]
    fn borrowed_kernel_proposal_store_records_without_consuming_kernel(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let proposal = valid_link_revision()?;
        let decision = ProposalPolicy::strict().evaluate(&proposal, created_at());
        let mut kernel = continuitydb_memory::MemoryKernel::default();

        {
            let store = BorrowedKernelProposalStore::new(&mut kernel);
            let mut ledger = StoredProposalLedger::new(store);
            ledger.record(proposal.clone(), decision)?;
        }

        let store = BorrowedKernelProposalStore::new(&mut kernel);
        let ledger = StoredProposalLedger::new(store);
        let record = ledger.record_by_id(proposal.id())?;

        assert_eq!(
            record.map(|record| record.proposal().id()),
            Some(proposal.id())
        );
        Ok(())
    }

    #[test]
    fn file_proposal_store_persists_records_across_reopen() -> Result<(), Box<dyn std::error::Error>>
    {
        let path = temp_proposal_store_path("continuitydb-proposal-store");
        let proposal = valid_link_revision()?;
        let decision = ProposalPolicy::strict().evaluate(&proposal, created_at());

        {
            let mut ledger = StoredProposalLedger::new(FileProposalStore::open(&path)?);
            ledger.record(proposal.clone(), decision)?;
        }

        let reopened = StoredProposalLedger::new(FileProposalStore::open(&path)?);
        let records = reopened.records()?;
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].proposal().id(), proposal.id());
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_proposal_store_gets_record_by_id_after_reopen() -> Result<(), Box<dyn std::error::Error>>
    {
        let path = temp_proposal_store_path("continuitydb-proposal-store-get");
        let proposal = valid_link_revision()?;
        let decision = ProposalPolicy::strict().evaluate(&proposal, created_at());

        StoredProposalLedger::new(FileProposalStore::open(&path)?)
            .record(proposal.clone(), decision)?;

        let reopened = StoredProposalLedger::new(FileProposalStore::open(&path)?);
        let record = reopened.record_by_id(proposal.id())?;
        assert_eq!(
            record.map(|record| record.proposal().id()),
            Some(proposal.id())
        );
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn file_proposal_store_rejects_invalid_jsonl() -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_proposal_store_path("continuitydb-proposal-store-invalid");
        fs::write(&path, "{not valid json}\n")?;
        let store = FileProposalStore::open(&path)?;

        let result = store.list_records();

        assert!(matches!(result, Err(StewardError::ProposalStoreCorrupt)));
        fs::remove_file(path)?;
        Ok(())
    }
}
