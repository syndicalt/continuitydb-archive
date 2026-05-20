//! Error types for Steward proposal handling.

use thiserror::Error;

/// Errors produced by Steward proposal construction or ledger recording.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum StewardError {
    /// Steward identity fields must be non-empty.
    #[error("steward identity fields must be non-empty")]
    EmptyStewardIdentity,
    /// Proposal rationale must be non-empty.
    #[error("proposal rationale must be non-empty")]
    EmptyRationale,
    /// Proposal citations must be non-empty.
    #[error("proposal must include at least one citation")]
    MissingCitations,
    /// Proposal decision IDs must match the recorded proposal.
    #[error("proposal decision ID does not match proposal ID")]
    MismatchedDecision,
    /// Local model response could not be decoded into Steward proposals.
    #[error("local model response is not a valid Steward proposal response")]
    InvalidModelResponse,
}
