//! Deterministic checkout and audit.

use chrono::{DateTime, Utc};
use continuitydb_core::{Confidence, Scope, StateCell, StateCellId};
use continuitydb_kernel::{CellLookup, KernelError, StorageKernel};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors produced by checkout.
#[derive(Debug, Error, PartialEq)]
pub enum CheckoutError {
    /// Storage kernel failure.
    #[error(transparent)]
    Kernel(#[from] KernelError),
}

/// Request constraints for deterministic checkout.
#[derive(Clone, Debug)]
pub struct CheckoutRequest {
    /// Optional scope filter.
    pub scope: Option<Scope>,
    /// Optional valid-time filter.
    pub valid_at: Option<DateTime<Utc>>,
    /// Minimum evidence confidence for included cells.
    pub minimum_confidence: Confidence,
    /// Maximum token budget for the returned slice.
    pub token_budget: i64,
}

/// Materialized continuity slice.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CheckoutSlice {
    /// Selected cells.
    pub cells: Vec<StateCell>,
    /// Total estimated tokens.
    pub total_tokens: i64,
}

/// Audit trace for a StateCell.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AuditTrace {
    /// Audited cell identifier.
    pub cell_id: StateCellId,
    /// Citation locators supporting the cell.
    pub citations: Vec<String>,
}

/// Materializes a deterministic continuity slice.
pub fn checkout<K: StorageKernel>(
    kernel: &K,
    request: CheckoutRequest,
) -> Result<CheckoutSlice, CheckoutError> {
    let mut candidates = kernel.lookup_cells(CellLookup {
        semantic_anchor: None,
        scope: request.scope,
        valid_at: request.valid_at,
        ..CellLookup::default()
    })?;

    candidates.retain(|cell| {
        cell.evidence
            .iter()
            .any(|evidence| evidence.confidence.value() >= request.minimum_confidence.value())
    });

    candidates.sort_by(|left, right| {
        let left_confidence = max_confidence(left);
        let right_confidence = max_confidence(right);
        right_confidence
            .partial_cmp(&left_confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut total_tokens = 0;
    let mut cells = Vec::new();
    for cell in candidates {
        let next_total = total_tokens + cell.cost.token_count;
        if next_total <= request.token_budget {
            total_tokens = next_total;
            cells.push(cell);
        }
    }

    Ok(CheckoutSlice {
        cells,
        total_tokens,
    })
}

/// Produces a basic audit trace for a StateCell.
pub fn audit(cell: &StateCell) -> AuditTrace {
    AuditTrace {
        cell_id: cell.id,
        citations: cell
            .evidence
            .iter()
            .map(|evidence| evidence.citation.locator.clone())
            .collect(),
    }
}

fn max_confidence(cell: &StateCell) -> f32 {
    cell.evidence
        .iter()
        .map(|evidence| evidence.confidence.value())
        .fold(0.0, f32::max)
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use continuitydb_core::{
        Answerability, CellCost, CellPayload, Citation, Confidence, Evidence, Scope,
        SemanticAnchor, SourceId, StateCell, StateCellId, TrustSignal, ValidTimeRange,
    };
    use continuitydb_kernel::StorageKernel;
    use continuitydb_memory::MemoryKernel;

    use super::{audit, checkout, CheckoutRequest};

    fn sample_cell(
        anchor: &str,
        confidence: f32,
        tokens: i64,
    ) -> Result<StateCell, Box<dyn std::error::Error>> {
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
                confidence: Confidence::new(confidence)?,
                trust: vec![TrustSignal::DirectObservation],
            }],
            CellPayload::Text(anchor.to_string()),
            CellCost::new(tokens, 0)?,
        )
        .map_err(Into::into)
    }

    #[test]
    fn checkout_respects_token_budget_and_confidence() -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let high = sample_cell("project:continuitydb:high", 0.95, 10)?;
        let low = sample_cell("project:continuitydb:low", 0.40, 10)?;
        kernel.append_cell(high.clone())?;
        kernel.append_cell(low)?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                scope: Some(Scope::Project("continuitydb".to_string())),
                valid_at: None,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 10,
            },
        )?;

        assert_eq!(slice.cells, vec![high]);
        assert_eq!(slice.total_tokens, 10);
        Ok(())
    }

    #[test]
    fn audit_includes_cell_id_and_citation_locator() -> Result<(), Box<dyn std::error::Error>> {
        let cell = sample_cell("project:continuitydb:audit", 0.95, 10)?;
        let trace = audit(&cell);

        assert_eq!(trace.cell_id, cell.id);
        assert_eq!(
            trace.citations,
            vec!["test://project:continuitydb:audit".to_string()]
        );
        Ok(())
    }
}
