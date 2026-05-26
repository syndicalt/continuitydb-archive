//! Core semantic types for ContinuityDB.

mod cell;
mod error;
mod evidence;
mod time;

pub use cell::{
    ActivationState, Answerability, AttentionSignal, CellCost, CellDependency, CellDependencyKind,
    CellPayload, CommitId, CommitManifest, ContextAbstractionLevel, ContextAffordance,
    ContextCompilerPolicy, ContextCompilerProposal, ContextCompilerProposalLine, ContextGap,
    ContextGapKind, ContextLifecycleEvaluation, ContextLifecyclePolicy, ContextLifecycleReason,
    ContextPacket, ContextPacketDependencyContext, ContextPacketEntry, ContextPacketEntrySource,
    ContextPacketOrigin, ContextPacketPlan, ContextPacketPurpose, ContextPacketRequirement,
    ContextPacketRevisionContext, ContextPacketRevisionRelation, ContextPacketSelection,
    ContextPacketSelectionReason, ContextPacketStrategy, ContextProfile, EpistemicAction,
    EpistemicActionReason, EpistemicCalibration, EpistemicExpectation, EpistemicPressure,
    EpistemicUncertainty, InvalidationCondition, InvalidationConditionKind, LifecycleStage,
    MemoryProjection, MemoryProjectionKind, PromotionPolicy, RetentionPolicy, RevisionLinkKind,
    RevisionLinkRecord, Scope, SemanticAnchor, StateCell, StateCellId, TrajectoryMemory, UsePolicy,
    UtilityFeedback,
};
pub use error::CoreError;
pub use evidence::{Citation, Confidence, Evidence, SourceId, TrustSignal};
pub use time::{SystemTimeRange, ValidTimeRange};

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    fn sample_state_cell(anchor: &str) -> Result<StateCell, Box<dyn std::error::Error>> {
        let valid_from = Utc
            .with_ymd_and_hms(2026, 5, 20, 0, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        StateCell::new(
            StateCellId::new(),
            vec![SemanticAnchor::new(anchor)],
            ValidTimeRange::new(valid_from, None)?,
            Scope::Project("continuitydb".to_string()),
            Answerability::new(vec!["what should the agent know?".to_string()])?,
            vec![Evidence {
                source: SourceId::new("test"),
                citation: Citation {
                    locator: format!("test://{anchor}"),
                },
                confidence: Confidence::new(0.8)?,
                trust: vec![TrustSignal::DirectObservation],
            }],
            CellPayload::Text(anchor.to_string()),
            CellCost::new(5, 0)?,
        )
        .map_err(Into::into)
    }

    #[test]
    fn state_cell_requires_anchor_and_evidence() -> Result<(), Box<dyn std::error::Error>> {
        let valid_from = Utc
            .with_ymd_and_hms(2026, 5, 20, 0, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let result = StateCell::new(
            StateCellId::new(),
            Vec::new(),
            ValidTimeRange::new(valid_from, None)?,
            Scope::Project("continuitydb".to_string()),
            Answerability::new(vec!["what is ContinuityDB?".to_string()])?,
            Vec::new(),
            CellPayload::Text("ContinuityDB is a datastore.".to_string()),
            CellCost::new(6, 0)?,
        );

        assert!(matches!(result, Err(CoreError::MissingSemanticAnchor)));
        Ok(())
    }

    #[test]
    fn confidence_is_bounded() {
        assert!(Confidence::new(0.0).is_ok());
        assert!(Confidence::new(1.0).is_ok());
        assert!(matches!(
            Confidence::new(1.1),
            Err(CoreError::ConfidenceOutOfRange { value }) if value == 1.1
        ));
    }

    #[test]
    fn commit_manifest_records_commit_boundary_and_ordered_cells(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let committed_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let commit_id = CommitId::new();
        let first = StateCellId::new();
        let second = StateCellId::new();

        let manifest = CommitManifest::new(commit_id, committed_at, vec![first, second]);

        assert_eq!(manifest.commit_id, commit_id);
        assert_eq!(manifest.committed_at, committed_at);
        assert_eq!(manifest.cell_ids, vec![first, second]);
        Ok(())
    }

    #[test]
    fn utility_feedback_score_averages_bounded_signals() -> Result<(), Box<dyn std::error::Error>> {
        let feedback = UtilityFeedback::new(
            Confidence::new(0.9)?,
            Confidence::new(0.6)?,
            Confidence::new(0.3)?,
        );

        assert!((feedback.utility_score() - 0.6).abs() < f32::EPSILON);
        Ok(())
    }

    #[test]
    fn state_cell_starts_with_neutral_utility_feedback() -> Result<(), Box<dyn std::error::Error>> {
        let valid_from = Utc
            .with_ymd_and_hms(2026, 5, 20, 0, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let cell = StateCell::new(
            StateCellId::new(),
            vec![SemanticAnchor::new("project:continuitydb:utility")],
            ValidTimeRange::new(valid_from, None)?,
            Scope::Project("continuitydb".to_string()),
            Answerability::new(vec!["what utility signals apply?".to_string()])?,
            vec![Evidence {
                source: SourceId::new("test"),
                citation: Citation {
                    locator: "test://utility".to_string(),
                },
                confidence: Confidence::new(0.8)?,
                trust: vec![TrustSignal::DirectObservation],
            }],
            CellPayload::Text("Utility feedback is tracked.".to_string()),
            CellCost::new(5, 0)?,
        )?;

        assert_eq!(cell.utility_feedback, UtilityFeedback::default());
        assert_eq!(cell.utility_feedback.utility_score(), 0.5);
        Ok(())
    }

    #[test]
    fn state_cell_deserializes_missing_utility_feedback_as_neutral(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let valid_from = Utc
            .with_ymd_and_hms(2026, 5, 20, 0, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let cell = StateCell::new(
            StateCellId::new(),
            vec![SemanticAnchor::new("project:continuitydb:legacy")],
            ValidTimeRange::new(valid_from, None)?,
            Scope::Project("continuitydb".to_string()),
            Answerability::new(vec!["what legacy records deserialize?".to_string()])?,
            vec![Evidence {
                source: SourceId::new("test"),
                citation: Citation {
                    locator: "test://legacy".to_string(),
                },
                confidence: Confidence::new(0.8)?,
                trust: vec![TrustSignal::DirectObservation],
            }],
            CellPayload::Text("Legacy cells omit utility feedback.".to_string()),
            CellCost::new(5, 0)?,
        )?;
        let mut value = serde_json::to_value(cell)?;
        value
            .as_object_mut()
            .ok_or_else(|| std::io::Error::other("state cell did not serialize as object"))?
            .remove("utility_feedback");

        let decoded: StateCell = serde_json::from_value(value)?;

        assert_eq!(decoded.utility_feedback, UtilityFeedback::default());
        Ok(())
    }

    #[test]
    fn cell_dependency_records_target_kind_and_rationale() {
        let target = StateCellId::new();
        let dependency = CellDependency::new(
            target,
            CellDependencyKind::DependsOn,
            "release status depends on verification evidence",
        );

        assert_eq!(dependency.target, target);
        assert_eq!(dependency.kind, CellDependencyKind::DependsOn);
        assert_eq!(
            dependency.rationale,
            "release status depends on verification evidence"
        );
    }

    #[test]
    fn revision_link_record_preserves_revision_relationship(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let source = StateCellId::from_u128(1);
        let target = StateCellId::from_u128(2);
        let recorded_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 13, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;

        let record =
            RevisionLinkRecord::new(source, RevisionLinkKind::Supersedes, target, recorded_at);
        let encoded = serde_json::to_string(&record)?;
        let decoded: RevisionLinkRecord = serde_json::from_str(&encoded)?;

        assert_eq!(decoded.source, source);
        assert_eq!(decoded.target, target);
        assert_eq!(decoded.kind, RevisionLinkKind::Supersedes);
        assert_eq!(decoded.recorded_at, recorded_at);
        Ok(())
    }

    #[test]
    fn state_cell_starts_with_empty_dependencies() -> Result<(), Box<dyn std::error::Error>> {
        let cell = sample_state_cell("project:continuitydb:dependencies")?;

        assert!(cell.dependencies.is_empty());
        Ok(())
    }

    #[test]
    fn state_cell_v2_defaults_to_observed_lifecycle_and_empty_projections(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let cell = sample_state_cell("project:continuitydb:v2-defaults")?;

        assert_eq!(cell.lifecycle_stage, LifecycleStage::Observed);
        assert!(cell.projections.is_empty());
        assert_eq!(cell.lifecycle_policy, ContextLifecyclePolicy::default());
        assert_eq!(
            cell.context_lifecycle_evaluation(),
            ContextLifecycleEvaluation::default()
        );
        assert_eq!(cell.uncertainty.score, Confidence::new(0.0)?);
        assert_eq!(cell.uncertainty.surprise_bits, 0.0);
        assert_eq!(cell.uncertainty.rationale, "");
        assert_eq!(cell.calibration.calibration_error, 0.0);
        assert_eq!(cell.calibration.rationale, "");
        assert_eq!(cell.attention.salience_score(), 0.0);
        assert!(cell.context_gaps.is_empty());
        assert!(cell.invalidation_conditions.is_empty());
        Ok(())
    }

    #[test]
    fn epistemic_uncertainty_requires_valid_surprise_and_rationale(
    ) -> Result<(), Box<dyn std::error::Error>> {
        assert!(matches!(
            EpistemicUncertainty::new(Confidence::new(0.3)?, -0.1, "conflicting evidence"),
            Err(CoreError::InvalidSurpriseBits { value }) if value == -0.1
        ));
        assert!(matches!(
            EpistemicUncertainty::new(Confidence::new(0.3)?, 1.2, " "),
            Err(CoreError::EmptyUncertaintyRationale)
        ));

        let uncertainty =
            EpistemicUncertainty::new(Confidence::new(0.73)?, 4.2, "baseline belief failed")?;

        assert_eq!(uncertainty.score, Confidence::new(0.73)?);
        assert_eq!(uncertainty.surprise_bits, 4.2);
        assert_eq!(uncertainty.rationale, "baseline belief failed");
        Ok(())
    }

    #[test]
    fn epistemic_expectation_trace_computes_surprise_from_baseline(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let failed_strong_expectation = EpistemicExpectation::from_expected_outcome(
            "release asset upload exists",
            Confidence::new(0.9)?,
            false,
        )?;
        let failed_uncertain_expectation = EpistemicExpectation::from_expected_outcome(
            "release asset upload exists",
            Confidence::new(0.5)?,
            false,
        )?;

        assert_eq!(
            failed_strong_expectation.label,
            "release asset upload exists"
        );
        assert_eq!(
            failed_strong_expectation.prior_probability,
            Confidence::new(0.9)?
        );
        assert_eq!(
            failed_strong_expectation.observed_probability,
            Confidence::new(0.1)?
        );
        assert_eq!(failed_strong_expectation.probability_delta, 0.8);
        assert_eq!(failed_uncertain_expectation.probability_delta, 0.0);
        assert!(
            (failed_strong_expectation.surprise_bits - std::f32::consts::LOG2_10).abs() < 0.000_01
        );
        assert_eq!(failed_uncertain_expectation.surprise_bits, 1.0);
        assert!(
            failed_strong_expectation.surprise_bits > failed_uncertain_expectation.surprise_bits
        );
        Ok(())
    }

    #[test]
    fn epistemic_uncertainty_can_derive_surprise_from_expectation_trace(
    ) -> Result<(), Box<dyn std::error::Error>> {
        assert!(matches!(
            EpistemicExpectation::from_expected_outcome(" ", Confidence::new(0.9)?, false),
            Err(CoreError::EmptyExpectationLabel)
        ));

        let expectation = EpistemicExpectation::from_expected_outcome(
            "release asset upload exists",
            Confidence::new(0.9)?,
            false,
        )?;
        let uncertainty = EpistemicUncertainty::from_expectation(
            Confidence::new(0.73)?,
            expectation.clone(),
            "baseline belief failed",
        )?;

        assert_eq!(uncertainty.score, Confidence::new(0.73)?);
        assert_eq!(uncertainty.expectation, Some(expectation.clone()));
        assert_eq!(uncertainty.surprise_bits, expectation.surprise_bits);
        assert_eq!(uncertainty.rationale, "baseline belief failed");
        Ok(())
    }

    #[test]
    fn epistemic_expectation_trace_deserializes_missing_probability_delta(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let expectation: EpistemicExpectation = serde_json::from_value(serde_json::json!({
            "label": "release asset upload exists",
            "prior_probability": 0.9,
            "observed_probability": 0.1,
            "surprise_bits": std::f32::consts::LOG2_10
        }))?;

        assert_eq!(expectation.label, "release asset upload exists");
        assert_eq!(expectation.prior_probability, Confidence::new(0.9)?);
        assert_eq!(expectation.observed_probability, Confidence::new(0.1)?);
        assert_eq!(expectation.probability_delta, 0.0);
        assert!((expectation.surprise_bits - std::f32::consts::LOG2_10).abs() < 0.000_01);
        Ok(())
    }

    #[test]
    fn state_cell_v2_context_packet_includes_native_uncertainty(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut cell = sample_state_cell("project:continuitydb:v2-native-uncertainty")?;
        cell.set_uncertainty(EpistemicUncertainty::new(
            Confidence::new(0.73)?,
            4.2,
            "baseline belief failed",
        )?);

        let packet = cell.context_packet(ContextProfile::Planning, 16);

        let origin = packet.origin.as_ref().ok_or_else(|| {
            std::io::Error::other("context packet did not preserve StateCell origin")
        })?;
        let selection = packet.selection.as_ref().ok_or_else(|| {
            std::io::Error::other("context packet did not preserve selection metadata")
        })?;
        assert_eq!(origin.cell_id, cell.id);
        assert_eq!(origin.anchors, cell.anchors);
        assert_eq!(origin.scope, Some(cell.scope.clone()));
        assert_eq!(origin.valid_time, Some(cell.valid_time.clone()));
        assert_eq!(origin.system_time, Some(cell.system_time.clone()));
        assert_eq!(origin.lifecycle_stage, LifecycleStage::Observed);
        assert_eq!(origin.activation, cell.activation);
        assert_eq!(origin.commit_id, cell.commit_id);
        assert_eq!(
            packet.citations,
            vec!["test://project:continuitydb:v2-native-uncertainty".to_string()]
        );
        assert_eq!(selection.max_confidence, Confidence::new(0.8)?);
        assert_eq!(selection.utility_score, 0.5);
        assert_eq!(selection.uncertainty_score, Confidence::new(0.73)?);
        assert_eq!(selection.surprise_bits, 4.2);
        assert!(selection
            .reasons
            .contains(&ContextPacketSelectionReason::NativeUncertainty));
        assert_eq!(
            packet.lines,
            vec!["Uncertainty 0.730, surprise 4.200 bits: baseline belief failed".to_string()]
        );
        assert_eq!(packet.entries.len(), 1);
        assert_eq!(
            packet.entries[0].source,
            ContextPacketEntrySource::NativeUncertainty
        );
        assert_eq!(packet.entries[0].confidence, Confidence::new(0.73)?);
        assert_eq!(packet.entries[0].token_count, 12);
        assert_eq!(packet.token_count, 12);
        Ok(())
    }

    #[test]
    fn state_cell_v2_context_packet_surfaces_expectation_trace(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut cell = sample_state_cell("project:continuitydb:v2-expectation-trace")?;
        let expectation = EpistemicExpectation::from_expected_outcome(
            "release asset upload exists",
            Confidence::new(0.9)?,
            false,
        )?;
        cell.set_uncertainty(EpistemicUncertainty::from_expectation(
            Confidence::new(0.73)?,
            expectation.clone(),
            "baseline belief failed",
        )?);

        let packet = cell.context_packet(ContextProfile::Planning, 24);
        let selection = packet.selection.as_ref().ok_or_else(|| {
            std::io::Error::other("context packet did not preserve selection metadata")
        })?;

        assert_eq!(selection.expectation, Some(expectation));
        assert_eq!(
            packet.lines,
            vec![
                "Uncertainty 0.730, surprise 3.322 bits: baseline belief failed".to_string(),
                "Expectation \"release asset upload exists\" prior 0.900, observed probability 0.100, probability delta 0.800"
                    .to_string(),
            ]
        );
        assert_eq!(packet.entries.len(), 2);
        assert_eq!(
            packet.entries[1].source,
            ContextPacketEntrySource::NativeExpectation
        );
        assert_eq!(packet.entries[1].confidence, Confidence::new(0.1)?);
        assert_eq!(packet.token_count, 18);
        Ok(())
    }

    #[test]
    fn state_cell_v2_context_packet_derives_scavenge_disposition_from_uncertain_surprise(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut cell = sample_state_cell("project:continuitydb:v2-scavenge-disposition")?;
        let expectation = EpistemicExpectation::from_expected_outcome(
            "release asset exists",
            Confidence::new(0.9)?,
            false,
        )?;
        cell.set_uncertainty(EpistemicUncertainty::from_expectation(
            Confidence::new(0.82)?,
            expectation,
            "strong baseline failed and needs missing evidence",
        )?);

        let packet = cell.context_packet(ContextProfile::Planning, 24);
        let selection = packet.selection.as_ref().ok_or_else(|| {
            std::io::Error::other("context packet did not preserve selection metadata")
        })?;

        assert_eq!(selection.epistemic_action, EpistemicAction::Scavenge);
        Ok(())
    }

    #[test]
    fn state_cell_v2_context_packet_explains_epistemic_action(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut cell = sample_state_cell("project:continuitydb:v2-action-rationale")?;
        let expectation = EpistemicExpectation::from_expected_outcome(
            "release asset exists",
            Confidence::new(0.9)?,
            false,
        )?;
        cell.set_uncertainty(EpistemicUncertainty::from_expectation(
            Confidence::new(0.82)?,
            expectation,
            "strong baseline failed and needs missing evidence",
        )?);

        let packet = cell.context_packet(ContextProfile::Planning, 24);
        let selection = packet.selection.as_ref().ok_or_else(|| {
            std::io::Error::other("context packet did not preserve selection metadata")
        })?;

        assert_eq!(selection.epistemic_action, EpistemicAction::Scavenge);
        assert_eq!(
            selection.epistemic_action_reasons,
            vec![
                EpistemicActionReason::HighUncertainty,
                EpistemicActionReason::HighSurprise
            ]
        );
        Ok(())
    }

    #[test]
    fn state_cell_v2_context_packet_surfaces_epistemic_pressure(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut cell = sample_state_cell("project:continuitydb:v2-epistemic-pressure")?;
        cell.evidence[0].confidence = Confidence::new(0.8)?;
        cell.set_uncertainty(EpistemicUncertainty::new(
            Confidence::new(0.7)?,
            4.0,
            "high uncertainty with violated baseline",
        )?);
        cell.set_attention(AttentionSignal::new(0.8, 0.6, 0.75, 0.4)?);

        let packet = cell.context_packet(ContextProfile::Planning, 24);
        let selection = packet.selection.as_ref().ok_or_else(|| {
            std::io::Error::other("context packet did not preserve selection metadata")
        })?;

        assert!((selection.epistemic_pressure.revision_pressure - 0.6).abs() < 0.000_01);
        assert!((selection.epistemic_pressure.scavenging_pressure - 0.573_125).abs() < 0.000_01);
        assert!((selection.epistemic_pressure.checkout_pressure - 0.6).abs() < 0.000_01);
        assert_eq!(cell.epistemic_pressure(), selection.epistemic_pressure);
        Ok(())
    }

    #[test]
    fn epistemic_calibration_requires_samples_and_rationale(
    ) -> Result<(), Box<dyn std::error::Error>> {
        assert!(matches!(
            EpistemicCalibration::new(
                Confidence::new(0.9)?,
                Confidence::new(0.55)?,
                0,
                "held-out checks exposed overconfidence"
            ),
            Err(CoreError::InvalidCalibrationSampleCount)
        ));
        assert!(matches!(
            EpistemicCalibration::new(Confidence::new(0.9)?, Confidence::new(0.55)?, 40, " "),
            Err(CoreError::EmptyCalibrationRationale)
        ));

        let calibration = EpistemicCalibration::new(
            Confidence::new(0.9)?,
            Confidence::new(0.55)?,
            40,
            "held-out checks exposed overconfidence",
        )?;

        assert_eq!(calibration.expected_confidence, Confidence::new(0.9)?);
        assert_eq!(calibration.observed_frequency, Confidence::new(0.55)?);
        assert_eq!(calibration.sample_count, 40);
        assert_eq!(calibration.calibration_error, 0.35);
        assert_eq!(
            calibration.rationale,
            "held-out checks exposed overconfidence"
        );
        Ok(())
    }

    #[test]
    fn state_cell_v2_context_packet_surfaces_epistemic_calibration(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut cell = sample_state_cell("project:continuitydb:v2-calibration")?;
        cell.set_calibration(EpistemicCalibration::new(
            Confidence::new(0.9)?,
            Confidence::new(0.55)?,
            40,
            "held-out checks exposed overconfidence",
        )?);

        let packet = cell.context_packet(ContextProfile::Planning, 12);
        let selection = packet.selection.as_ref().ok_or_else(|| {
            std::io::Error::other("context packet did not preserve selection metadata")
        })?;

        assert_eq!(selection.calibration, cell.calibration);
        assert_eq!(selection.calibration_error, 0.35);
        assert!(selection
            .reasons
            .contains(&ContextPacketSelectionReason::EpistemicCalibration));
        assert_eq!(
            packet.lines,
            vec![
                "Calibration expected confidence 0.900, observed frequency 0.550, error 0.350 over 40 samples: held-out checks exposed overconfidence"
                    .to_string()
            ]
        );
        assert_eq!(packet.entries.len(), 1);
        assert_eq!(
            packet.entries[0].source,
            ContextPacketEntrySource::NativeCalibration
        );
        assert_eq!(packet.entries[0].confidence, Confidence::new(0.55)?);
        assert_eq!(packet.entries[0].token_count, 12);
        assert_eq!(packet.token_count, 12);
        Ok(())
    }

    #[test]
    fn state_cell_v2_calibration_error_drives_verification_disposition(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut cell = sample_state_cell("project:continuitydb:v2-calibration-action")?;
        cell.set_calibration(EpistemicCalibration::new(
            Confidence::new(0.92)?,
            Confidence::new(0.50)?,
            30,
            "source has been overconfident on similar claims",
        )?);

        let packet = cell.context_packet(ContextProfile::Planning, 16);
        let selection = packet.selection.as_ref().ok_or_else(|| {
            std::io::Error::other("context packet did not preserve selection metadata")
        })?;

        assert_eq!(selection.epistemic_action, EpistemicAction::Verify);
        assert!(selection
            .epistemic_action_reasons
            .contains(&EpistemicActionReason::MiscalibratedConfidence));
        Ok(())
    }

    #[test]
    fn state_cell_v2_context_lifecycle_policy_evaluates_use_and_promotion(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut cell = sample_state_cell("project:continuitydb:v2-lifecycle-policy")?;
        cell.set_lifecycle_policy(ContextLifecyclePolicy {
            retention: RetentionPolicy::DecayUnlessReinforced,
            use_policy: UsePolicy::VerifyBeforeUse,
            promotion: PromotionPolicy::ConfidenceThreshold(Confidence::new(0.75)?),
        });

        let evaluation = cell.context_lifecycle_evaluation();

        assert_eq!(evaluation.retention, RetentionPolicy::DecayUnlessReinforced);
        assert_eq!(evaluation.use_policy, UsePolicy::VerifyBeforeUse);
        assert_eq!(
            evaluation.promotion,
            PromotionPolicy::ConfidenceThreshold(Confidence::new(0.75)?)
        );
        assert_eq!(
            evaluation.reasons,
            vec![
                ContextLifecycleReason::DecayUnlessReinforced,
                ContextLifecycleReason::VerifyBeforeUse,
                ContextLifecycleReason::PromotionThresholdMet,
            ]
        );
        Ok(())
    }

    #[test]
    fn state_cell_v2_context_packet_surfaces_lifecycle_policy(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut cell = sample_state_cell("project:continuitydb:v2-policy-packet")?;
        cell.set_lifecycle_policy(ContextLifecyclePolicy {
            retention: RetentionPolicy::DecayUnlessReinforced,
            use_policy: UsePolicy::HedgeBeforeUse,
            promotion: PromotionPolicy::Manual,
        });

        let packet = cell.context_packet(ContextProfile::Planning, 12);
        let selection = packet.selection.as_ref().ok_or_else(|| {
            std::io::Error::other("context packet did not preserve selection metadata")
        })?;

        assert_eq!(
            selection.lifecycle_policy,
            cell.context_lifecycle_evaluation()
        );
        assert!(selection
            .reasons
            .contains(&ContextPacketSelectionReason::LifecyclePolicy));
        assert_eq!(
            packet.lines,
            vec![
                "Lifecycle policy: decay unless reinforced; hedge before use; manual promotion"
                    .to_string()
            ]
        );
        assert_eq!(packet.entries.len(), 1);
        assert_eq!(
            packet.entries[0].source,
            ContextPacketEntrySource::NativeLifecyclePolicy
        );
        assert_eq!(packet.entries[0].token_count, 10);
        Ok(())
    }

    #[test]
    fn attention_signal_requires_bounded_components_and_reports_salience(
    ) -> Result<(), Box<dyn std::error::Error>> {
        assert!(matches!(
            AttentionSignal::new(1.1, 0.2, 0.3, 0.4),
            Err(CoreError::AttentionSignalOutOfRange { value }) if value == 1.1
        ));
        assert!(matches!(
            AttentionSignal::new(0.1, f32::NAN, 0.3, 0.4),
            Err(CoreError::AttentionSignalOutOfRange { value }) if value.is_nan()
        ));

        let attention = AttentionSignal::new(0.9, 0.7, 0.8, 0.2)?;

        assert_eq!(attention.novelty, 0.9);
        assert_eq!(attention.urgency, 0.7);
        assert_eq!(attention.impact, 0.8);
        assert_eq!(attention.decay_resistance, 0.2);
        assert!((attention.salience_score() - 0.65).abs() < 0.000_01);
        Ok(())
    }

    #[test]
    fn context_affordance_requires_bounded_components_and_scores_value_of_context(
    ) -> Result<(), Box<dyn std::error::Error>> {
        assert!(matches!(
            ContextAffordance::new(1.1, 0.2, 0.3, 0.4, 0.5, 0.6),
            Err(CoreError::ContextAffordanceOutOfRange { value }) if value == 1.1
        ));
        assert!(matches!(
            ContextAffordance::new(0.1, f32::NAN, 0.3, 0.4, 0.5, 0.6),
            Err(CoreError::ContextAffordanceOutOfRange { value }) if value.is_nan()
        ));

        let affordance = ContextAffordance::new(0.9, 0.8, 0.7, 0.4, 0.6, 0.2)?;

        assert_eq!(affordance.expected_task_value, 0.9);
        assert_eq!(affordance.expected_information_gain, 0.8);
        assert_eq!(affordance.risk_of_misuse, 0.7);
        assert_eq!(affordance.ambiguity, 0.4);
        assert_eq!(affordance.applicability, 0.6);
        assert_eq!(affordance.resource_pressure, 0.2);
        assert!((affordance.context_affordance_score() - 0.544).abs() < 0.000_01);
        Ok(())
    }

    #[test]
    fn state_cell_v2_context_packet_surfaces_context_affordance(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut cell = sample_state_cell("project:continuitydb:v2-context-affordance")?;
        cell.set_context_affordance(ContextAffordance::new(0.9, 0.8, 0.7, 0.4, 0.6, 0.2)?);

        let packet = cell.context_packet(ContextProfile::Planning, 12);
        let selection = packet.selection.as_ref().ok_or_else(|| {
            std::io::Error::other("context packet did not preserve selection metadata")
        })?;

        assert_eq!(selection.context_affordance, cell.context_affordance);
        assert!((selection.context_affordance_score - 0.544).abs() < 0.000_01);
        assert!(selection
            .reasons
            .contains(&ContextPacketSelectionReason::ContextAffordance));
        assert_eq!(
            packet.lines,
            vec![
                "Context affordance 0.544: task value 0.900, information gain 0.800, misuse risk 0.700, ambiguity 0.400, applicability 0.600, resource pressure 0.200"
                    .to_string()
            ]
        );
        assert_eq!(packet.entries.len(), 1);
        assert_eq!(
            packet.entries[0].source,
            ContextPacketEntrySource::NativeContextAffordance
        );
        assert_eq!(packet.entries[0].token_count, 12);
        assert_eq!(packet.token_count, 12);
        Ok(())
    }

    #[test]
    fn state_cell_v2_context_packet_surfaces_attention_signal(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut cell = sample_state_cell("project:continuitydb:v2-attention")?;
        cell.set_attention(AttentionSignal::new(0.9, 0.7, 0.8, 0.2)?);

        let packet = cell.context_packet(ContextProfile::Planning, 12);
        let selection = packet.selection.as_ref().ok_or_else(|| {
            std::io::Error::other("context packet did not preserve selection metadata")
        })?;

        assert_eq!(selection.attention, cell.attention);
        assert!((selection.salience_score - 0.65).abs() < 0.000_01);
        assert!(selection
            .reasons
            .contains(&ContextPacketSelectionReason::AttentionSignal));
        assert_eq!(
            packet.lines,
            vec![
                "Attention salience 0.650: novelty 0.900, urgency 0.700, impact 0.800, decay resistance 0.200"
                    .to_string()
            ]
        );
        assert_eq!(packet.entries.len(), 1);
        assert_eq!(
            packet.entries[0].source,
            ContextPacketEntrySource::NativeAttention
        );
        assert_eq!(packet.entries[0].token_count, 10);
        assert_eq!(packet.token_count, 10);
        Ok(())
    }

    #[test]
    fn context_gap_requires_question_rationale_and_bounded_priority(
    ) -> Result<(), Box<dyn std::error::Error>> {
        assert!(matches!(
            ContextGap::new(ContextGapKind::MissingEvidence, " ", "need source", 0.7),
            Err(CoreError::EmptyContextGapQuestion)
        ));
        assert!(matches!(
            ContextGap::new(
                ContextGapKind::MissingDecision,
                "which release failed?",
                " ",
                0.7
            ),
            Err(CoreError::EmptyContextGapRationale)
        ));
        assert!(matches!(
            ContextGap::new(
                ContextGapKind::MissingConstraint,
                "which token budget applies?",
                "benchmark scope needs narrowing",
                1.1
            ),
            Err(CoreError::ContextGapPriorityOutOfRange { value }) if value == 1.1
        ));

        let gap = ContextGap::new(
            ContextGapKind::MissingEvidence,
            "which artifact proves the release upload?",
            "uncertain release status needs concrete retained evidence",
            0.9,
        )?;

        assert_eq!(gap.kind, ContextGapKind::MissingEvidence);
        assert_eq!(gap.question, "which artifact proves the release upload?");
        assert_eq!(
            gap.rationale,
            "uncertain release status needs concrete retained evidence"
        );
        assert_eq!(gap.priority, Confidence::new(0.9)?);
        Ok(())
    }

    #[test]
    fn state_cell_v2_context_packet_surfaces_context_gaps() -> Result<(), Box<dyn std::error::Error>>
    {
        let mut cell = sample_state_cell("project:continuitydb:v2-context-gap")?;
        cell.add_context_gap(ContextGap::new(
            ContextGapKind::MissingEvidence,
            "which retained artifact proves the live run?",
            "avoid collapsing uncertainty into unsupported confidence",
            0.9,
        )?);

        let packet = cell.context_packet(ContextProfile::Planning, 16);
        let selection = packet.selection.as_ref().ok_or_else(|| {
            std::io::Error::other("context packet did not preserve selection metadata")
        })?;

        assert_eq!(selection.context_gaps, cell.context_gaps);
        assert!(selection
            .reasons
            .contains(&ContextPacketSelectionReason::ContextGap));
        assert_eq!(
            packet.lines,
            vec![
                "Context gap missing evidence priority 0.900: which retained artifact proves the live run? Rationale: avoid collapsing uncertainty into unsupported confidence"
                    .to_string()
            ]
        );
        assert_eq!(packet.entries.len(), 1);
        assert_eq!(
            packet.entries[0].source,
            ContextPacketEntrySource::NativeContextGap
        );
        assert_eq!(packet.entries[0].confidence, Confidence::new(0.9)?);
        assert_eq!(packet.entries[0].token_count, 12);
        assert_eq!(packet.token_count, 12);
        Ok(())
    }

    #[test]
    fn trajectory_memory_requires_reusable_experience_contract(
    ) -> Result<(), Box<dyn std::error::Error>> {
        assert!(matches!(
            TrajectoryMemory::new(
                " ",
                "implemented upload-report validation",
                "release upload failed with HTTP 404",
                "artifact://rollout/upload-failure",
                0.8,
                "verify the GitHub release before uploading assets",
                vec!["GitHub Release asset publication".to_string()],
                vec!["release ID changes or artifact digest changes".to_string()],
                ContextPacketStrategy::ScavengingBrief,
            ),
            Err(CoreError::EmptyTrajectoryHypothesis)
        ));
        assert!(matches!(
            TrajectoryMemory::new(
                "upload release assets after build",
                "implemented upload-report validation",
                "release upload failed with HTTP 404",
                " ",
                0.8,
                "verify the GitHub release before uploading assets",
                vec!["GitHub Release asset publication".to_string()],
                vec!["release ID changes or artifact digest changes".to_string()],
                ContextPacketStrategy::ScavengingBrief,
            ),
            Err(CoreError::EmptyTrajectoryTraceLocator)
        ));
        assert!(matches!(
            TrajectoryMemory::new(
                "upload release assets after build",
                "implemented upload-report validation",
                "release upload failed with HTTP 404",
                "artifact://rollout/upload-failure",
                0.8,
                "verify the GitHub release before uploading assets",
                vec![" ".to_string()],
                vec!["release ID changes or artifact digest changes".to_string()],
                ContextPacketStrategy::ScavengingBrief,
            ),
            Err(CoreError::EmptyTrajectoryApplicabilityCondition)
        ));

        let memory = TrajectoryMemory::new(
            "upload release assets after build",
            "implemented upload-report validation",
            "release upload failed with HTTP 404",
            "artifact://rollout/upload-failure",
            0.8,
            "verify the GitHub release before uploading assets",
            vec!["GitHub Release asset publication".to_string()],
            vec!["release ID changes or artifact digest changes".to_string()],
            ContextPacketStrategy::ScavengingBrief,
        )?;

        assert_eq!(memory.hypothesis_tried, "upload release assets after build");
        assert_eq!(memory.progress_made, "implemented upload-report validation");
        assert_eq!(memory.failure_mode, "release upload failed with HTTP 404");
        assert_eq!(memory.trace_locator, "artifact://rollout/upload-failure");
        assert_eq!(memory.confidence, Confidence::new(0.8)?);
        assert_eq!(
            memory.reusable_lesson,
            "verify the GitHub release before uploading assets"
        );
        assert_eq!(
            memory.applicability_conditions,
            vec!["GitHub Release asset publication".to_string()]
        );
        assert_eq!(
            memory.invalidation_conditions,
            vec!["release ID changes or artifact digest changes".to_string()]
        );
        assert_eq!(
            memory.checkout_strategy,
            ContextPacketStrategy::ScavengingBrief
        );
        Ok(())
    }

    #[test]
    fn state_cell_v2_context_packet_surfaces_trajectory_memory(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut cell = sample_state_cell("project:continuitydb:v2-trajectory-memory")?;
        cell.set_trajectory_memory(TrajectoryMemory::new(
            "retry release upload after workflow fix",
            "release preflight and upload reports were retained",
            "upload failed because the target GitHub Release was missing",
            "artifact://rollout/release-upload-404",
            0.86,
            "ensure the release exists before uploading retained assets",
            vec!["publishing release assets from CI".to_string()],
            vec!["release lookup succeeds and upload report validates".to_string()],
            ContextPacketStrategy::ScavengingBrief,
        )?);

        let packet = cell.context_packet(ContextProfile::Reflection, 20);
        let selection = packet.selection.as_ref().ok_or_else(|| {
            std::io::Error::other("context packet did not preserve selection metadata")
        })?;

        assert_eq!(selection.trajectory_memory, cell.trajectory_memory);
        assert!(selection
            .reasons
            .contains(&ContextPacketSelectionReason::TrajectoryMemory));
        assert_eq!(
            packet.lines,
            vec![
                "Trajectory lesson confidence 0.860: ensure the release exists before uploading retained assets. Tried: retry release upload after workflow fix. Progress: release preflight and upload reports were retained. Failure: upload failed because the target GitHub Release was missing. Applies when: publishing release assets from CI. Invalidated when: release lookup succeeds and upload report validates. Trace: artifact://rollout/release-upload-404"
                    .to_string()
            ]
        );
        assert_eq!(packet.entries.len(), 1);
        assert_eq!(
            packet.entries[0].source,
            ContextPacketEntrySource::NativeTrajectoryMemory
        );
        assert_eq!(packet.entries[0].confidence, Confidence::new(0.86)?);
        assert_eq!(packet.entries[0].token_count, 16);
        assert_eq!(packet.token_count, 16);
        Ok(())
    }

    #[test]
    fn invalidation_condition_requires_condition_rationale_and_bounded_priority(
    ) -> Result<(), Box<dyn std::error::Error>> {
        assert!(matches!(
            InvalidationCondition::new(
                InvalidationConditionKind::ContradictoryEvidence,
                " ",
                "need a concrete falsifier",
                0.8
            ),
            Err(CoreError::EmptyInvalidationCondition)
        ));
        assert!(matches!(
            InvalidationCondition::new(
                InvalidationConditionKind::BoundaryViolation,
                "release artifact digest no longer matches manifest",
                " ",
                0.8
            ),
            Err(CoreError::EmptyInvalidationRationale)
        ));
        assert!(matches!(
            InvalidationCondition::new(
                InvalidationConditionKind::TemporalExpiry,
                "deployment window has passed",
                "time-bounded operational facts should decay",
                1.1
            ),
            Err(CoreError::InvalidationPriorityOutOfRange { value }) if value == 1.1
        ));

        let condition = InvalidationCondition::new(
            InvalidationConditionKind::DependencyInvalidated,
            "the source benchmark artifact is superseded",
            "derived conclusions should be revised when their evidence changes",
            0.9,
        )?;

        assert_eq!(
            condition.kind,
            InvalidationConditionKind::DependencyInvalidated
        );
        assert_eq!(
            condition.condition,
            "the source benchmark artifact is superseded"
        );
        assert_eq!(
            condition.rationale,
            "derived conclusions should be revised when their evidence changes"
        );
        assert_eq!(condition.priority, Confidence::new(0.9)?);
        Ok(())
    }

    #[test]
    fn state_cell_v2_context_packet_surfaces_invalidation_conditions(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut cell = sample_state_cell("project:continuitydb:v2-invalidation")?;
        cell.add_invalidation_condition(InvalidationCondition::new(
            InvalidationConditionKind::ContradictoryEvidence,
            "a retained live benchmark artifact contradicts this conclusion",
            "beliefs need explicit falsifiers to prevent stale confident context",
            0.95,
        )?);

        let packet = cell.context_packet(ContextProfile::Planning, 16);
        let selection = packet.selection.as_ref().ok_or_else(|| {
            std::io::Error::other("context packet did not preserve selection metadata")
        })?;

        assert_eq!(
            selection.invalidation_conditions,
            cell.invalidation_conditions
        );
        assert!(selection
            .reasons
            .contains(&ContextPacketSelectionReason::InvalidationCondition));
        assert_eq!(
            packet.lines,
            vec![
                "Invalidation condition contradictory evidence priority 0.950: a retained live benchmark artifact contradicts this conclusion Rationale: beliefs need explicit falsifiers to prevent stale confident context"
                    .to_string()
            ]
        );
        assert_eq!(packet.entries.len(), 1);
        assert_eq!(
            packet.entries[0].source,
            ContextPacketEntrySource::NativeInvalidationCondition
        );
        assert_eq!(packet.entries[0].confidence, Confidence::new(0.95)?);
        assert_eq!(packet.entries[0].token_count, 12);
        assert_eq!(packet.token_count, 12);
        Ok(())
    }

    #[test]
    fn memory_projection_rejects_empty_text() -> Result<(), Box<dyn std::error::Error>> {
        let result = MemoryProjection::new(
            MemoryProjectionKind::Semantic,
            " ",
            Confidence::new(0.8)?,
            CellCost::new(1, 0)?,
        );

        assert!(matches!(result, Err(CoreError::EmptyProjection)));
        Ok(())
    }

    #[test]
    fn state_cell_v2_stores_lifecycle_and_memory_form_projections(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut cell = sample_state_cell("project:continuitydb:v2-projections")?
            .with_lifecycle_stage(LifecycleStage::Operationalized);
        cell.add_projection(MemoryProjection::new(
            MemoryProjectionKind::Episodic,
            "Release upload failed with HTTP 404.",
            Confidence::new(0.9)?,
            CellCost::new(8, 0)?,
        )?);
        cell.add_projection(MemoryProjection::new(
            MemoryProjectionKind::Procedural,
            "Before publishing, verify GitHub release assets directly.",
            Confidence::new(0.85)?,
            CellCost::new(9, 0)?,
        )?);

        assert_eq!(cell.lifecycle_stage, LifecycleStage::Operationalized);
        assert_eq!(cell.projections.len(), 2);
        assert_eq!(
            cell.projections_by_kind(MemoryProjectionKind::Procedural)
                .first()
                .map(|projection| projection.text.as_str()),
            Some("Before publishing, verify GitHub release assets directly.")
        );
        Ok(())
    }

    #[test]
    fn state_cell_v2_context_packet_selects_task_relevant_projections_under_budget(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut cell = sample_state_cell("project:continuitydb:v2-context-packet")?
            .with_lifecycle_stage(LifecycleStage::Operationalized);
        cell.add_projection(MemoryProjection::new(
            MemoryProjectionKind::Episodic,
            "Earlier workflow claimed upload success.",
            Confidence::new(0.7)?,
            CellCost::new(8, 0)?,
        )?);
        cell.add_projection(MemoryProjection::new(
            MemoryProjectionKind::Semantic,
            "Current belief: asset-present after rerun.",
            Confidence::new(0.91)?,
            CellCost::new(8, 0)?,
        )?);
        cell.add_projection(MemoryProjection::new(
            MemoryProjectionKind::Procedural,
            "Safest action: verify checksum provenance before broader release claims.",
            Confidence::new(0.86)?,
            CellCost::new(10, 0)?,
        )?);
        cell.add_projection(MemoryProjection::new(
            MemoryProjectionKind::Uncertainty,
            "Active risk: checksum provenance unknown.",
            Confidence::new(0.62)?,
            CellCost::new(7, 0)?,
        )?);

        let packet = cell.context_packet(ContextProfile::Execution, 18);

        assert_eq!(packet.profile, ContextProfile::Execution);
        assert_eq!(
            packet.origin.as_ref().map(|origin| origin.lifecycle_stage),
            Some(LifecycleStage::Operationalized)
        );
        let selection = packet.selection.as_ref().ok_or_else(|| {
            std::io::Error::other("context packet did not preserve selection metadata")
        })?;
        assert_eq!(selection.max_confidence, Confidence::new(0.8)?);
        assert_eq!(selection.utility_score, 0.5);
        assert_eq!(selection.uncertainty_score, Confidence::new(0.0)?);
        assert_eq!(selection.surprise_bits, 0.0);
        assert_eq!(
            selection.reasons,
            vec![
                ContextPacketSelectionReason::EvidenceConfidence,
                ContextPacketSelectionReason::LifecycleStage,
                ContextPacketSelectionReason::ProjectionProfile,
                ContextPacketSelectionReason::Answerability,
            ]
        );
        assert!(packet.token_count <= 18);
        assert_eq!(
            packet
                .entries
                .iter()
                .map(|entry| (&entry.source, entry.text.as_str(), entry.confidence))
                .collect::<Vec<_>>(),
            vec![
                (
                    &ContextPacketEntrySource::Projection(MemoryProjectionKind::Semantic),
                    "Current belief: asset-present after rerun.",
                    Confidence::new(0.91)?
                ),
                (
                    &ContextPacketEntrySource::Projection(MemoryProjectionKind::Procedural),
                    "Safest action: verify checksum provenance before broader release claims.",
                    Confidence::new(0.86)?
                )
            ]
        );
        assert_eq!(
            packet.lines,
            vec![
                "Current belief: asset-present after rerun.".to_string(),
                "Safest action: verify checksum provenance before broader release claims."
                    .to_string(),
            ]
        );
        Ok(())
    }

    #[test]
    fn state_cell_starts_with_default_system_time() -> Result<(), Box<dyn std::error::Error>> {
        let cell = sample_state_cell("project:continuitydb:system-time")?;
        let epoch = Utc
            .timestamp_opt(0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid epoch timestamp"))?;

        assert_eq!(cell.system_time.from(), epoch);
        assert!(cell.system_time.contains(epoch));
        Ok(())
    }

    #[test]
    fn state_cell_starts_with_nil_commit_id() -> Result<(), Box<dyn std::error::Error>> {
        let cell = sample_state_cell("project:continuitydb:commit-id")?;

        assert_eq!(cell.commit_id, CommitId::nil());
        assert!(cell.commit_id.is_nil());
        Ok(())
    }

    #[test]
    fn commit_id_new_is_not_nil() {
        let commit_id = CommitId::new();

        assert!(!commit_id.is_nil());
        assert_ne!(commit_id, CommitId::nil());
    }

    #[test]
    fn commit_id_displays_and_parses_uuid_text() -> Result<(), Box<dyn std::error::Error>> {
        let commit_id = CommitId::new();
        let text = commit_id.to_string();

        let parsed: CommitId = text.parse()?;

        assert_eq!(parsed, commit_id);
        assert_eq!(text.len(), 36);
        Ok(())
    }

    #[test]
    fn commit_id_rejects_invalid_uuid_text() {
        let result = "not-a-uuid".parse::<CommitId>();

        assert!(result.is_err());
    }

    #[test]
    fn state_cell_id_displays_and_parses_uuid_text() -> Result<(), Box<dyn std::error::Error>> {
        let cell_id = StateCellId::new();
        let text = cell_id.to_string();

        let parsed: StateCellId = text.parse()?;

        assert_eq!(parsed, cell_id);
        Ok(())
    }

    #[test]
    fn state_cell_id_from_u128_is_stable() -> Result<(), Box<dyn std::error::Error>> {
        let cell_id = StateCellId::from_u128(42);
        let repeated = StateCellId::from_u128(42);
        let different = StateCellId::from_u128(43);
        let parsed: StateCellId = cell_id.to_string().parse()?;

        assert_eq!(cell_id, repeated);
        assert_eq!(parsed, cell_id);
        assert_ne!(cell_id, different);
        Ok(())
    }

    #[test]
    fn state_cell_id_rejects_invalid_uuid_text() {
        let result = "not-a-uuid".parse::<StateCellId>();

        assert!(result.is_err());
    }

    #[test]
    fn state_cell_deserializes_missing_commit_id_as_nil() -> Result<(), Box<dyn std::error::Error>>
    {
        let cell = sample_state_cell("project:continuitydb:legacy-commit-id")?;
        let mut value = serde_json::to_value(cell)?;
        value
            .as_object_mut()
            .ok_or_else(|| std::io::Error::other("state cell did not serialize as object"))?
            .remove("commit_id");

        let decoded: StateCell = serde_json::from_value(value)?;

        assert_eq!(decoded.commit_id, CommitId::nil());
        Ok(())
    }

    #[test]
    fn state_cell_deserializes_missing_dependencies_as_empty(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let cell = sample_state_cell("project:continuitydb:legacy-dependencies")?;
        let mut value = serde_json::to_value(cell)?;
        value
            .as_object_mut()
            .ok_or_else(|| std::io::Error::other("state cell did not serialize as object"))?
            .remove("dependencies");

        let decoded: StateCell = serde_json::from_value(value)?;

        assert!(decoded.dependencies.is_empty());
        Ok(())
    }

    #[test]
    fn state_cell_deserializes_missing_lifecycle_policy_as_default(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let cell = sample_state_cell("project:continuitydb:legacy-lifecycle-policy")?;
        let mut value = serde_json::to_value(cell)?;
        value
            .as_object_mut()
            .ok_or_else(|| std::io::Error::other("state cell did not serialize as object"))?
            .remove("lifecycle_policy");

        let decoded: StateCell = serde_json::from_value(value)?;

        assert_eq!(decoded.lifecycle_policy, ContextLifecyclePolicy::default());
        Ok(())
    }

    #[test]
    fn context_packet_deserializes_missing_dependency_context_as_empty(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let packet = sample_state_cell("project:continuitydb:legacy-packet-dependencies")?
            .context_packet(ContextProfile::Execution, 8);
        let mut value = serde_json::to_value(packet)?;
        value
            .as_object_mut()
            .ok_or_else(|| std::io::Error::other("context packet did not serialize as object"))?
            .remove("dependency_context");

        let decoded: ContextPacket = serde_json::from_value(value)?;

        assert!(decoded.dependency_context.is_empty());
        Ok(())
    }

    #[test]
    fn context_packet_deserializes_missing_citations_as_empty(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let packet = sample_state_cell("project:continuitydb:legacy-packet-citations")?
            .context_packet(ContextProfile::Execution, 8);
        let mut value = serde_json::to_value(packet)?;
        value
            .as_object_mut()
            .ok_or_else(|| std::io::Error::other("context packet did not serialize as object"))?
            .remove("citations");

        let decoded: ContextPacket = serde_json::from_value(value)?;

        assert!(decoded.citations.is_empty());
        Ok(())
    }

    #[test]
    fn context_packet_deserializes_missing_strategy_as_raw_projection(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let packet = sample_state_cell("project:continuitydb:legacy-packet-strategy")?
            .context_packet(ContextProfile::Execution, 8);
        let mut value = serde_json::to_value(packet)?;
        value
            .as_object_mut()
            .ok_or_else(|| std::io::Error::other("context packet did not serialize as object"))?
            .remove("strategy");

        let decoded: ContextPacket = serde_json::from_value(value)?;

        assert_eq!(decoded.strategy, ContextPacketStrategy::RawProjection);
        Ok(())
    }

    #[test]
    fn state_cell_context_packet_defaults_to_raw_projection_strategy(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let cell = sample_state_cell("project:continuitydb:raw-packet-strategy")?;

        let packet = cell.context_packet(ContextProfile::Execution, 8);

        assert_eq!(packet.strategy, ContextPacketStrategy::RawProjection);
        Ok(())
    }

    #[test]
    fn context_packet_deserializes_missing_compiler_policy_as_automatic(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let packet = sample_state_cell("project:continuitydb:legacy-packet-policy")?
            .context_packet(ContextProfile::Execution, 8);
        let mut value = serde_json::to_value(packet)?;
        value
            .as_object_mut()
            .ok_or_else(|| std::io::Error::other("context packet did not serialize as object"))?
            .remove("compiler_policy");

        let decoded: ContextPacket = serde_json::from_value(value)?;

        assert_eq!(decoded.compiler_policy, ContextCompilerPolicy::Automatic);
        Ok(())
    }

    #[test]
    fn context_packet_deserializes_missing_abstraction_level_as_raw(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let packet = sample_state_cell("project:continuitydb:legacy-packet-abstraction")?
            .context_packet(ContextProfile::Execution, 8);
        let mut value = serde_json::to_value(packet)?;
        value
            .as_object_mut()
            .ok_or_else(|| std::io::Error::other("context packet did not serialize as object"))?
            .remove("abstraction_level");

        let decoded: ContextPacket = serde_json::from_value(value)?;

        assert_eq!(decoded.abstraction_level, ContextAbstractionLevel::Raw);
        Ok(())
    }

    #[test]
    fn context_packet_deserializes_missing_compiler_reason_tags_as_empty(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let packet = sample_state_cell("project:continuitydb:legacy-packet-compiler-reasons")?
            .context_packet(ContextProfile::Execution, 8);
        let mut value = serde_json::to_value(packet)?;
        value
            .as_object_mut()
            .ok_or_else(|| std::io::Error::other("context packet did not serialize as object"))?
            .remove("compiler_reason_tags");

        let decoded: ContextPacket = serde_json::from_value(value)?;

        assert!(decoded.compiler_reason_tags.is_empty());
        Ok(())
    }

    #[test]
    fn context_packet_deserializes_missing_compiler_evidence_locators_as_empty(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let packet = sample_state_cell("project:continuitydb:legacy-packet-compiler-evidence")?
            .context_packet(ContextProfile::Execution, 8);
        let mut value = serde_json::to_value(packet)?;
        value
            .as_object_mut()
            .ok_or_else(|| std::io::Error::other("context packet did not serialize as object"))?
            .remove("compiler_evidence_locators");

        let decoded: ContextPacket = serde_json::from_value(value)?;

        assert!(decoded.compiler_evidence_locators.is_empty());
        Ok(())
    }

    #[test]
    fn context_packet_plan_round_trips_with_required_contracts(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let plan = ContextPacketPlan::new(
            ContextPacketPurpose::RevisionGuidance,
            vec![
                ContextPacketRequirement::RevisionContext,
                ContextPacketRequirement::EvidenceCitations,
            ],
            vec![ContextPacketRequirement::LifecyclePolicy],
            ContextAbstractionLevel::Capsule,
            ContextPacketStrategy::RevisionCapsule,
            vec![
                "superseded-state".to_string(),
                "revision-guidance".to_string(),
            ],
            vec![
                "test://revision-context".to_string(),
                "test://supporting-evidence".to_string(),
            ],
        )?;

        let encoded = serde_json::to_string(&plan)?;
        let decoded: ContextPacketPlan = serde_json::from_str(&encoded)?;

        assert_eq!(decoded, plan);
        assert_eq!(decoded.purpose, ContextPacketPurpose::RevisionGuidance);
        assert_eq!(
            decoded.required_contracts,
            vec![
                ContextPacketRequirement::RevisionContext,
                ContextPacketRequirement::EvidenceCitations
            ]
        );
        Ok(())
    }

    #[test]
    fn context_packet_plan_requires_reason_tags_and_evidence_locators() {
        let missing_reasons = ContextPacketPlan::new(
            ContextPacketPurpose::SafetyGuidance,
            vec![ContextPacketRequirement::Invalidation],
            Vec::new(),
            ContextAbstractionLevel::Falsification,
            ContextPacketStrategy::FalsificationBrief,
            Vec::new(),
            vec!["test://safety-evidence".to_string()],
        );
        let missing_evidence = ContextPacketPlan::new(
            ContextPacketPurpose::SafetyGuidance,
            vec![ContextPacketRequirement::Invalidation],
            Vec::new(),
            ContextAbstractionLevel::Falsification,
            ContextPacketStrategy::FalsificationBrief,
            vec!["unsafe-assumption".to_string()],
            Vec::new(),
        );

        assert_eq!(
            missing_reasons,
            Err(CoreError::EmptyContextCompilerProposalReason)
        );
        assert_eq!(
            missing_evidence,
            Err(CoreError::EmptyContextCompilerProposalEvidence)
        );
    }

    #[test]
    fn context_packet_deserializes_missing_plan_as_absent() -> Result<(), Box<dyn std::error::Error>>
    {
        let packet = sample_state_cell("project:continuitydb:legacy-packet-plan")?
            .context_packet(ContextProfile::Execution, 8);
        let mut value = serde_json::to_value(packet)?;
        value
            .as_object_mut()
            .ok_or_else(|| std::io::Error::other("context packet did not serialize as object"))?
            .remove("plan");

        let decoded: ContextPacket = serde_json::from_value(value)?;

        assert_eq!(decoded.plan, None);
        Ok(())
    }

    #[test]
    fn state_cell_context_packet_preserves_packet_plan() -> Result<(), Box<dyn std::error::Error>> {
        let mut cell = sample_state_cell("project:continuitydb:planned-packet")?;
        cell.set_uncertainty(EpistemicUncertainty::new(
            Confidence::new(0.72)?,
            2.4,
            "source evidence is incomplete",
        )?);

        let packet = cell.context_packet(ContextProfile::Planning, 16);
        let plan = packet
            .plan
            .ok_or_else(|| std::io::Error::other("context packet did not preserve plan"))?;

        assert_eq!(plan.purpose, ContextPacketPurpose::UncertaintyGuidance);
        assert!(plan
            .required_contracts
            .contains(&ContextPacketRequirement::Uncertainty));
        assert!(plan
            .required_contracts
            .contains(&ContextPacketRequirement::EvidenceCitations));
        assert_eq!(plan.strategy, ContextPacketStrategy::UncertaintyBrief);
        assert_eq!(plan.abstraction_level, ContextAbstractionLevel::Brief);
        assert_eq!(plan.evidence_locators, packet.citations);
        assert!(!plan.reason_tags.is_empty());
        Ok(())
    }

    #[test]
    fn state_cell_context_packet_omits_plan_when_evidence_locators_are_absent(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let cell = sample_state_cell("project:continuitydb:missing-plan-evidence")?;
        let mut value = serde_json::to_value(cell)?;
        value
            .as_object_mut()
            .ok_or_else(|| std::io::Error::other("state cell did not serialize as object"))?
            .insert("evidence".to_string(), serde_json::json!([]));
        let decoded: StateCell = serde_json::from_value(value)?;

        let packet = decoded.context_packet(ContextProfile::Execution, 8);

        assert_eq!(packet.plan, None);
        Ok(())
    }

    #[test]
    fn context_compiler_proposal_requires_evidence_locators(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let result = ContextCompilerProposal::new(
            StateCellId::new(),
            ContextPacketStrategy::FalsificationBrief,
            ContextAbstractionLevel::Falsification,
            vec!["unsafe-action-intent".to_string()],
            Vec::new(),
        );

        assert_eq!(result, Err(CoreError::EmptyContextCompilerProposalEvidence));
        Ok(())
    }

    #[test]
    fn context_compiler_proposal_deduplicates_reason_tags_and_evidence_locators(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let proposal = ContextCompilerProposal::new(
            StateCellId::new(),
            ContextPacketStrategy::FalsificationBrief,
            ContextAbstractionLevel::Falsification,
            vec![
                "trajectory-reuse".to_string(),
                "safety-guidance".to_string(),
                "trajectory-reuse".to_string(),
            ],
            vec![
                "artifact://rollout/release-upload-404".to_string(),
                "test://supporting-evidence".to_string(),
                "artifact://rollout/release-upload-404".to_string(),
            ],
        )?;

        assert_eq!(
            proposal.reason_tags,
            &[
                "trajectory-reuse".to_string(),
                "safety-guidance".to_string()
            ]
        );
        assert_eq!(
            proposal.evidence_locators,
            &[
                "artifact://rollout/release-upload-404".to_string(),
                "test://supporting-evidence".to_string()
            ]
        );
        Ok(())
    }

    #[test]
    fn context_compiler_proposal_lines_require_citations() -> Result<(), Box<dyn std::error::Error>>
    {
        let result = ContextCompilerProposalLine::new(
            "Use the retained upload failure before retrying.",
            Vec::new(),
            6,
        );

        assert_eq!(
            result,
            Err(CoreError::EmptyContextCompilerProposalLineCitation)
        );
        Ok(())
    }

    #[test]
    fn context_compiler_proposal_line_deduplicates_citations(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let line = ContextCompilerProposalLine::new(
            "Use the retained upload failure before retrying.",
            vec![
                "artifact://rollout/release-upload-404".to_string(),
                "test://supporting-evidence".to_string(),
                "artifact://rollout/release-upload-404".to_string(),
            ],
            6,
        )?;

        assert_eq!(
            line.citations,
            &[
                "artifact://rollout/release-upload-404".to_string(),
                "test://supporting-evidence".to_string()
            ]
        );
        Ok(())
    }

    #[test]
    fn context_compiler_proposal_accepts_citation_backed_lines(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let line = ContextCompilerProposalLine::new(
            "Use the retained upload failure before retrying.",
            vec!["test://project:continuitydb:model-assisted-line".to_string()],
            6,
        )?;
        let proposal = ContextCompilerProposal::new(
            StateCellId::new(),
            ContextPacketStrategy::FalsificationBrief,
            ContextAbstractionLevel::Falsification,
            vec!["model-assisted-line".to_string()],
            vec!["artifact://compiler/proposal/line".to_string()],
        )?
        .with_proposed_lines(vec![line.clone()]);

        assert_eq!(proposal.proposed_lines, vec![line]);
        Ok(())
    }

    #[test]
    fn context_compiler_proposal_deduplicates_proposed_lines(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let first_line = ContextCompilerProposalLine::new(
            "Verify the release target before retrying upload.",
            vec!["artifact://rollout/release-upload-404".to_string()],
            7,
        )?;
        let second_line = ContextCompilerProposalLine::new(
            "Check retained upload evidence before declaring success.",
            vec!["test://supporting-evidence".to_string()],
            8,
        )?;
        let proposal = ContextCompilerProposal::new(
            StateCellId::new(),
            ContextPacketStrategy::FalsificationBrief,
            ContextAbstractionLevel::Falsification,
            vec!["model-assisted-line".to_string()],
            vec!["artifact://compiler/proposal/line".to_string()],
        )?
        .with_proposed_lines(vec![
            first_line.clone(),
            second_line.clone(),
            first_line.clone(),
        ]);

        assert_eq!(proposal.proposed_lines, vec![first_line, second_line]);
        Ok(())
    }

    #[test]
    fn context_compiler_raw_projection_proposal_drops_proposed_lines(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let line = ContextCompilerProposalLine::new(
            "Raw projection must not retain model-written guidance.",
            vec!["test://raw-projection-evidence".to_string()],
            7,
        )?;
        let proposal = ContextCompilerProposal::new(
            StateCellId::new(),
            ContextPacketStrategy::RawProjection,
            ContextAbstractionLevel::Raw,
            vec!["model-assisted-raw".to_string()],
            vec!["test://raw-projection-evidence".to_string()],
        )?
        .with_proposed_lines(vec![line]);

        assert!(proposal.proposed_lines.is_empty());
        Ok(())
    }

    #[test]
    fn context_compiler_proposal_rejects_incompatible_packet_shape(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let result = ContextCompilerProposal::new(
            StateCellId::new(),
            ContextPacketStrategy::FalsificationBrief,
            ContextAbstractionLevel::Raw,
            vec!["model-assisted-falsification".to_string()],
            vec!["artifact://compiler/proposal/shape".to_string()],
        );

        assert_eq!(result, Err(CoreError::InvalidContextCompilerProposalShape));
        Ok(())
    }

    #[test]
    fn context_compiler_proposal_accepts_compatible_packet_shapes(
    ) -> Result<(), Box<dyn std::error::Error>> {
        for (strategy, abstraction_level) in [
            (
                ContextPacketStrategy::RawProjection,
                ContextAbstractionLevel::Raw,
            ),
            (
                ContextPacketStrategy::OperationalBrief,
                ContextAbstractionLevel::Brief,
            ),
            (
                ContextPacketStrategy::RevisionCapsule,
                ContextAbstractionLevel::Capsule,
            ),
            (
                ContextPacketStrategy::UncertaintyBrief,
                ContextAbstractionLevel::Brief,
            ),
            (
                ContextPacketStrategy::ScavengingBrief,
                ContextAbstractionLevel::Scavenging,
            ),
            (
                ContextPacketStrategy::FalsificationBrief,
                ContextAbstractionLevel::Falsification,
            ),
        ] {
            let proposal = ContextCompilerProposal::new(
                StateCellId::new(),
                strategy,
                abstraction_level,
                vec!["compatible-shape".to_string()],
                vec!["artifact://compiler/proposal/compatible".to_string()],
            )?;

            assert_eq!(proposal.strategy, strategy);
            assert_eq!(proposal.abstraction_level, abstraction_level);
        }
        Ok(())
    }

    #[test]
    fn context_compiler_proposal_accepts_evidence_dense_non_raw_packet_shapes(
    ) -> Result<(), Box<dyn std::error::Error>> {
        for strategy in [
            ContextPacketStrategy::OperationalBrief,
            ContextPacketStrategy::RevisionCapsule,
            ContextPacketStrategy::UncertaintyBrief,
            ContextPacketStrategy::ScavengingBrief,
            ContextPacketStrategy::FalsificationBrief,
        ] {
            let proposal = ContextCompilerProposal::new(
                StateCellId::new(),
                strategy,
                ContextAbstractionLevel::EvidenceDense,
                vec!["audit-context-needed".to_string()],
                vec!["artifact://compiler/proposal/evidence-dense".to_string()],
            )?;

            assert_eq!(proposal.strategy, strategy);
            assert_eq!(
                proposal.abstraction_level,
                ContextAbstractionLevel::EvidenceDense
            );
        }
        Ok(())
    }

    #[test]
    fn context_packet_selection_deserializes_missing_epistemic_action_as_use(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let packet = sample_state_cell("project:continuitydb:legacy-selection-action")?
            .context_packet(ContextProfile::Execution, 8);
        let mut value = serde_json::to_value(packet)?;
        let selection = value
            .get_mut("selection")
            .and_then(|selection| selection.as_object_mut())
            .ok_or_else(|| std::io::Error::other("context packet selection did not serialize"))?;
        selection.remove("epistemic_action");

        let decoded: ContextPacket = serde_json::from_value(value)?;
        let decoded_selection = decoded
            .selection
            .ok_or_else(|| std::io::Error::other("missing decoded selection"))?;

        assert_eq!(decoded_selection.epistemic_action, EpistemicAction::Use);
        Ok(())
    }

    #[test]
    fn context_packet_selection_deserializes_missing_epistemic_action_reasons_as_empty(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let packet = sample_state_cell("project:continuitydb:legacy-selection-action-reasons")?
            .context_packet(ContextProfile::Execution, 8);
        let mut value = serde_json::to_value(packet)?;
        let selection = value
            .get_mut("selection")
            .and_then(|selection| selection.as_object_mut())
            .ok_or_else(|| std::io::Error::other("context packet selection did not serialize"))?;
        selection.remove("epistemic_action_reasons");

        let decoded: ContextPacket = serde_json::from_value(value)?;
        let decoded_selection = decoded
            .selection
            .ok_or_else(|| std::io::Error::other("missing decoded selection"))?;

        assert!(decoded_selection.epistemic_action_reasons.is_empty());
        Ok(())
    }

    #[test]
    fn context_packet_origin_deserializes_missing_scope_and_time_as_absent(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let packet = sample_state_cell("project:continuitydb:legacy-packet-origin")?
            .context_packet(ContextProfile::Execution, 8);
        let mut value = serde_json::to_value(packet)?;
        let origin = value
            .get_mut("origin")
            .and_then(|origin| origin.as_object_mut())
            .ok_or_else(|| std::io::Error::other("context packet origin did not serialize"))?;
        origin.remove("scope");
        origin.remove("valid_time");
        origin.remove("system_time");

        let decoded: ContextPacket = serde_json::from_value(value)?;
        let decoded_origin = decoded
            .origin
            .ok_or_else(|| std::io::Error::other("missing decoded origin"))?;

        assert_eq!(decoded_origin.scope, None);
        assert_eq!(decoded_origin.valid_time, None);
        assert_eq!(decoded_origin.system_time, None);
        Ok(())
    }

    #[test]
    fn source_id_as_str_returns_stored_identifier() {
        let source = SourceId::new("human-review");

        assert_eq!(source.as_str(), "human-review");
    }

    #[test]
    fn answerability_questions_returns_normalized_questions(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let answerability = Answerability::new(vec![
            " what is frontier? ".to_string(),
            " ".to_string(),
            "what needs verification?".to_string(),
        ])?;

        assert_eq!(
            answerability.questions(),
            &[
                "what is frontier?".to_string(),
                "what needs verification?".to_string()
            ]
        );
        Ok(())
    }

    #[test]
    fn valid_time_contains_as_of_in_half_open_range() -> Result<(), Box<dyn std::error::Error>> {
        let start = Utc
            .with_ymd_and_hms(2026, 5, 20, 0, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let end = Utc
            .with_ymd_and_hms(2026, 5, 21, 0, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let range = ValidTimeRange::new(start, Some(end))?;

        assert!(range.contains(start));
        assert!(!range.contains(end));
        Ok(())
    }

    #[test]
    fn valid_time_overlap_respects_half_open_ranges() -> Result<(), Box<dyn std::error::Error>> {
        let may_20 = Utc
            .with_ymd_and_hms(2026, 5, 20, 0, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let may_21 = Utc
            .with_ymd_and_hms(2026, 5, 21, 0, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let may_22 = Utc
            .with_ymd_and_hms(2026, 5, 22, 0, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let may_23 = Utc
            .with_ymd_and_hms(2026, 5, 23, 0, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let current = ValidTimeRange::new(may_20, Some(may_22))?;
        let overlapping = ValidTimeRange::new(may_21, Some(may_23))?;
        let adjacent = ValidTimeRange::new(may_22, Some(may_23))?;

        assert!(current.overlaps(&overlapping));
        assert!(!current.overlaps(&adjacent));
        Ok(())
    }

    #[test]
    fn valid_time_exposes_start() -> Result<(), Box<dyn std::error::Error>> {
        let may_20 = Utc
            .with_ymd_and_hms(2026, 5, 20, 0, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let may_21 = Utc
            .with_ymd_and_hms(2026, 5, 21, 0, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let range = ValidTimeRange::new(may_20, Some(may_21))?;

        assert_eq!(range.from(), may_20);
        Ok(())
    }
}
