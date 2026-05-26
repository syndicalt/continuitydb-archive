//! Error types for core ContinuityDB validation.

use thiserror::Error;

/// Errors produced when constructing invalid core values.
#[derive(Clone, Debug, Error, PartialEq)]
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
    /// Memory projections must include non-empty text.
    #[error("memory projection requires non-empty text")]
    EmptyProjection,
    /// Native uncertainty rationale must be non-empty when uncertainty is recorded.
    #[error("epistemic uncertainty requires non-empty rationale")]
    EmptyUncertaintyRationale,
    /// Epistemic expectation traces must name the expected outcome.
    #[error("epistemic expectation requires non-empty label")]
    EmptyExpectationLabel,
    /// Surprise bits must be finite and non-negative.
    #[error("surprise bits must be finite and non-negative, got {value}")]
    InvalidSurpriseBits {
        /// The invalid surprise value.
        value: f32,
    },
    /// Calibration traces must include at least one observation.
    #[error("epistemic calibration requires at least one observation")]
    InvalidCalibrationSampleCount,
    /// Calibration traces must explain the reliability signal.
    #[error("epistemic calibration requires non-empty rationale")]
    EmptyCalibrationRationale,
    /// Attention signal components must be finite and bounded.
    #[error("attention signal component must be between 0.0 and 1.0, got {value}")]
    AttentionSignalOutOfRange {
        /// The invalid attention component.
        value: f32,
    },
    /// Context affordance components must be finite and bounded.
    #[error("context affordance component must be between 0.0 and 1.0, got {value}")]
    ContextAffordanceOutOfRange {
        /// The invalid context affordance component.
        value: f32,
    },
    /// Context gaps must include the missing-context question.
    #[error("context gap requires non-empty question")]
    EmptyContextGapQuestion,
    /// Context gaps must explain why the missing context matters.
    #[error("context gap requires non-empty rationale")]
    EmptyContextGapRationale,
    /// Context gap priority must be finite and bounded.
    #[error("context gap priority must be between 0.0 and 1.0, got {value}")]
    ContextGapPriorityOutOfRange {
        /// The invalid context gap priority.
        value: f32,
    },
    /// Invalidation conditions must state the falsifying condition.
    #[error("invalidation condition requires non-empty condition")]
    EmptyInvalidationCondition,
    /// Invalidation conditions must explain why invalidation matters.
    #[error("invalidation condition requires non-empty rationale")]
    EmptyInvalidationRationale,
    /// Invalidation condition priority must be finite and bounded.
    #[error("invalidation condition priority must be between 0.0 and 1.0, got {value}")]
    InvalidationPriorityOutOfRange {
        /// The invalid invalidation priority.
        value: f32,
    },
    /// Trajectory memories must describe the hypothesis that was tried.
    #[error("trajectory memory requires non-empty hypothesis")]
    EmptyTrajectoryHypothesis,
    /// Trajectory memories must describe progress made during the rollout.
    #[error("trajectory memory requires non-empty progress")]
    EmptyTrajectoryProgress,
    /// Trajectory memories must describe the rollout failure mode.
    #[error("trajectory memory requires non-empty failure mode")]
    EmptyTrajectoryFailureMode,
    /// Trajectory memories must cite the retained rollout trace.
    #[error("trajectory memory requires non-empty trace locator")]
    EmptyTrajectoryTraceLocator,
    /// Trajectory memories must include a reusable lesson.
    #[error("trajectory memory requires non-empty reusable lesson")]
    EmptyTrajectoryReusableLesson,
    /// Trajectory memories must include non-empty applicability conditions.
    #[error("trajectory memory requires non-empty applicability condition")]
    EmptyTrajectoryApplicabilityCondition,
    /// Trajectory memories must include non-empty invalidation conditions.
    #[error("trajectory memory requires non-empty invalidation condition")]
    EmptyTrajectoryInvalidationCondition,
    /// Token and compute costs must be non-negative.
    #[error("cell costs must be non-negative")]
    InvalidCost,
    /// Model-proposed context compiler choices require at least one reason tag.
    #[error("context compiler proposal requires at least one non-empty reason tag")]
    EmptyContextCompilerProposalReason,
    /// Model-proposed context compiler choices require evidence locators.
    #[error("context compiler proposal requires at least one non-empty evidence locator")]
    EmptyContextCompilerProposalEvidence,
    /// Model-proposed context compiler choices must use a compatible strategy and abstraction.
    #[error("context compiler proposal strategy and abstraction level are incompatible")]
    InvalidContextCompilerProposalShape,
    /// Model-proposed context compiler packet lines require non-empty text.
    #[error("context compiler proposal line requires non-empty text")]
    EmptyContextCompilerProposalLine,
    /// Model-proposed context compiler packet lines require at least one source citation.
    #[error("context compiler proposal line requires at least one non-empty citation")]
    EmptyContextCompilerProposalLineCitation,
}
