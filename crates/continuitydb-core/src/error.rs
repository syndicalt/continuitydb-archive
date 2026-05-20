//! Error types for core ContinuityDB validation.

use thiserror::Error;

/// Errors produced when constructing invalid core values.
#[derive(Debug, Error, PartialEq)]
pub enum CoreError {
    /// A StateCell must have at least one semantic anchor.
    #[error("state cell requires at least one semantic anchor")]
    MissingSemanticAnchor,
    /// A StateCell must have at least one evidence record.
    #[error("state cell requires at least one evidence record")]
    MissingEvidence,
    /// Confidence values must be between zero and one, inclusive.
    #[error("confidence must be between 0.0 and 1.0, got {value}")]
    ConfidenceOutOfRange {
        /// The invalid confidence value.
        value: f32,
    },
    /// Time ranges must end after they start.
    #[error("time range end must be after start")]
    InvalidTimeRange,
    /// Answerability must include at least one non-empty question.
    #[error("answerability requires at least one non-empty question")]
    EmptyAnswerability,
    /// Token and compute costs must be non-negative.
    #[error("cell costs must be non-negative")]
    InvalidCost,
}
