//! Core semantic types for ContinuityDB.

mod cell;
mod error;
mod evidence;
mod time;

pub use cell::{
    ActivationState, Answerability, CellCost, CellDependency, CellDependencyKind, CellPayload,
    Scope, SemanticAnchor, StateCell, StateCellId, UtilityFeedback,
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
    fn state_cell_starts_with_empty_dependencies() -> Result<(), Box<dyn std::error::Error>> {
        let cell = sample_state_cell("project:continuitydb:dependencies")?;

        assert!(cell.dependencies.is_empty());
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
}
