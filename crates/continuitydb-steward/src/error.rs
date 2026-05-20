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
    /// Local model executable failed to produce a usable response.
    #[error("local model executable failed")]
    LocalModelExecutionFailed,
    /// Local model benchmark baseline store I/O failed.
    #[error("local model benchmark baseline store I/O failed")]
    LocalModelBenchmarkBaselineStoreIo,
    /// Local model benchmark baseline store content could not be decoded.
    #[error("local model benchmark baseline store content is corrupt")]
    LocalModelBenchmarkBaselineStoreCorrupt,
    /// Frontier subscription fields must be non-empty.
    #[error("frontier subscription must include signals and citation")]
    EmptyFrontierSubscription,
    /// Frontier subscription store I/O failed.
    #[error("frontier subscription store I/O failed")]
    FrontierSubscriptionStoreIo,
    /// Frontier subscription store content could not be decoded.
    #[error("frontier subscription store content is corrupt")]
    FrontierSubscriptionStoreCorrupt,
    /// Proposal store I/O failed.
    #[error("proposal store I/O failed")]
    ProposalStoreIo,
    /// Proposal store content could not be decoded.
    #[error("proposal store content is corrupt")]
    ProposalStoreCorrupt,
}
