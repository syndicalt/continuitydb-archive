//! Bitemporal range types.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::CoreError;

/// Real-world validity range for a StateCell version.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ValidTimeRange {
    from: DateTime<Utc>,
    to: Option<DateTime<Utc>>,
}

impl ValidTimeRange {
    /// Creates a valid-time range. `to` is exclusive when present.
    pub fn new(from: DateTime<Utc>, to: Option<DateTime<Utc>>) -> Result<Self, CoreError> {
        if let Some(to) = to {
            if to <= from {
                return Err(CoreError::InvalidTimeRange);
            }
        }

        Ok(Self { from, to })
    }

    /// Returns true when `as_of` falls inside the half-open valid-time range.
    pub fn contains(&self, as_of: DateTime<Utc>) -> bool {
        as_of >= self.from && self.to.map_or(true, |to| as_of < to)
    }
}

/// System transaction-time range for a StateCell version.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SystemTimeRange {
    from: DateTime<Utc>,
    to: Option<DateTime<Utc>>,
}

impl SystemTimeRange {
    /// Creates a system-time range. `to` is exclusive when present.
    pub fn new(from: DateTime<Utc>, to: Option<DateTime<Utc>>) -> Result<Self, CoreError> {
        if let Some(to) = to {
            if to <= from {
                return Err(CoreError::InvalidTimeRange);
            }
        }

        Ok(Self { from, to })
    }
}
