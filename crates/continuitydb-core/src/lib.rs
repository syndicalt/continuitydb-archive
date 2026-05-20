//! Core semantic types for ContinuityDB.

mod cell;
mod error;
mod evidence;
mod time;

pub use cell::{
    ActivationState, Answerability, CellCost, CellPayload, Scope, SemanticAnchor, StateCell,
    StateCellId,
};
pub use error::CoreError;
pub use evidence::{Citation, Confidence, Evidence, SourceId, TrustSignal};
pub use time::{SystemTimeRange, ValidTimeRange};

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

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
}
