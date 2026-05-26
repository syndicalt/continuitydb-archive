//! Evidence and provenance types.

use serde::{Deserialize, Serialize};

use crate::CoreError;

/// Stable identifier for an evidence source.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct SourceId(String);

impl SourceId {
    /// Creates a source identifier from a string.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the inner source identifier.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Human or machine-readable citation for evidence.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Citation {
    /// Source-local citation URI, path, or durable locator.
    pub locator: String,
}

/// Confidence score in the inclusive range 0.0..=1.0.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Confidence(f32);

impl Confidence {
    /// Lowest valid confidence value.
    pub const ZERO: Self = Self(0.0);

    /// Creates a bounded confidence value.
    pub fn new(value: f32) -> Result<Self, CoreError> {
        if !(0.0..=1.0).contains(&value) {
            return Err(CoreError::ConfidenceOutOfRange { value });
        }

        Ok(Self(value))
    }

    /// Returns the inner scalar confidence value.
    pub fn value(self) -> f32 {
        self.0
    }
}

impl Default for Confidence {
    fn default() -> Self {
        Self(0.5)
    }
}

/// Signal about source trust or evidence quality.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum TrustSignal {
    /// Evidence was directly observed.
    DirectObservation,
    /// Evidence was inferred from other evidence.
    Derived,
    /// Evidence was supplied by a human operator.
    HumanSupplied,
}

/// Evidence supporting a StateCell version.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Evidence {
    /// Evidence source identity.
    pub source: SourceId,
    /// Citation pointing to the evidence.
    pub citation: Citation,
    /// Confidence assigned to this evidence.
    pub confidence: Confidence,
    /// Trust signals associated with this evidence.
    pub trust: Vec<TrustSignal>,
}
