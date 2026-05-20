//! Deterministic checkout and audit.

use chrono::{DateTime, Utc};
use continuitydb_core::{ActivationState, Confidence, CoreError, Scope, StateCell, StateCellId};
use continuitydb_kernel::{CellLookup, KernelError, StorageKernel};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors produced by checkout.
#[derive(Debug, Error, PartialEq)]
pub enum CheckoutError {
    /// Storage kernel failure.
    #[error(transparent)]
    Kernel(#[from] KernelError),
    /// Checkout metadata could not be derived from selected cells.
    #[error(transparent)]
    Core(#[from] CoreError),
}

/// Request constraints for deterministic checkout.
#[derive(Clone, Debug)]
pub struct CheckoutRequest {
    /// Optional scope filter.
    pub scope: Option<Scope>,
    /// Optional valid-time filter.
    pub valid_at: Option<DateTime<Utc>>,
    /// Optional exact answerability question filter.
    pub answerability_question: Option<String>,
    /// Optional exact evidence-source filter.
    pub evidence_source: Option<String>,
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
    /// Citation traces for selected cells.
    pub audit_traces: Vec<AuditTrace>,
    /// Per-cell uncertainty metadata for selected cells.
    pub uncertainty: Vec<UncertaintyEntry>,
    /// Selected frontier cells that should remain monitored.
    pub frontier_recommendations: Vec<FrontierRecommendation>,
    /// Candidate cells that matched constraints but were not selected.
    pub alternatives: Vec<CheckoutAlternative>,
}

/// Deterministic uncertainty metadata for a selected StateCell.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UncertaintyEntry {
    /// Selected cell identifier.
    pub cell_id: StateCellId,
    /// Maximum confidence across the selected cell's evidence.
    pub max_confidence: Confidence,
}

/// Recommendation to keep a selected frontier StateCell under monitoring.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FrontierRecommendation {
    /// Frontier cell identifier.
    pub cell_id: StateCellId,
    /// Citation locators supporting the frontier recommendation.
    pub citations: Vec<String>,
}

/// Deterministic reason a matching candidate was not selected.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CheckoutAlternativeReason {
    /// Candidate could not fit inside the remaining token budget.
    TokenBudgetExceeded,
}

/// Metadata for a matching candidate omitted from the selected slice.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CheckoutAlternative {
    /// Omitted candidate cell identifier.
    pub cell_id: StateCellId,
    /// Deterministic omission reason.
    pub reason: CheckoutAlternativeReason,
    /// Citation locators supporting the omitted candidate.
    pub citations: Vec<String>,
    /// Maximum confidence across the omitted candidate's evidence.
    pub max_confidence: Confidence,
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
        answerability_question: request.answerability_question,
        evidence_source: request.evidence_source,
        minimum_confidence: Some(request.minimum_confidence),
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
    let mut alternatives = Vec::new();
    for cell in candidates {
        let next_total = total_tokens + cell.cost.token_count;
        if next_total <= request.token_budget {
            total_tokens = next_total;
            cells.push(cell);
        } else {
            alternatives.push(CheckoutAlternative {
                cell_id: cell.id,
                reason: CheckoutAlternativeReason::TokenBudgetExceeded,
                citations: citations(&cell),
                max_confidence: Confidence::new(max_confidence(&cell))?,
            });
        }
    }

    let audit_traces: Vec<AuditTrace> = cells.iter().map(audit).collect();
    let uncertainty = cells
        .iter()
        .map(|cell| {
            Ok(UncertaintyEntry {
                cell_id: cell.id,
                max_confidence: Confidence::new(max_confidence(cell))?,
            })
        })
        .collect::<Result<Vec<_>, CheckoutError>>()?;
    let frontier_recommendations = cells
        .iter()
        .filter(|cell| cell.activation == ActivationState::Frontier)
        .map(|cell| FrontierRecommendation {
            cell_id: cell.id,
            citations: citations(cell),
        })
        .collect();

    Ok(CheckoutSlice {
        cells,
        total_tokens,
        audit_traces,
        uncertainty,
        frontier_recommendations,
        alternatives,
    })
}

/// Produces a basic audit trace for a StateCell.
pub fn audit(cell: &StateCell) -> AuditTrace {
    AuditTrace {
        cell_id: cell.id,
        citations: citations(cell),
    }
}

fn citations(cell: &StateCell) -> Vec<String> {
    cell.evidence
        .iter()
        .map(|evidence| evidence.citation.locator.clone())
        .collect()
}

fn max_confidence(cell: &StateCell) -> f32 {
    cell.evidence
        .iter()
        .map(|evidence| evidence.confidence.value())
        .fold(0.0, f32::max)
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use chrono::{TimeZone, Utc};
    use continuitydb_core::{
        ActivationState, Answerability, CellCost, CellPayload, Citation, Confidence, Evidence,
        Scope, SemanticAnchor, SourceId, StateCell, StateCellId, TrustSignal, ValidTimeRange,
    };
    use continuitydb_kernel::{CellLookup, KernelError, StorageKernel};
    use continuitydb_memory::MemoryKernel;

    use super::{audit, checkout, CheckoutRequest};

    fn sample_cell(
        anchor: &str,
        confidence: f32,
        tokens: i64,
    ) -> Result<StateCell, Box<dyn std::error::Error>> {
        sample_cell_with_question_and_source(
            anchor,
            "what should the agent know?",
            "test",
            confidence,
            tokens,
        )
    }

    fn sample_cell_with_question_and_source(
        anchor: &str,
        question: &str,
        source: &str,
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
            Answerability::new(vec![question.to_string()])?,
            vec![Evidence {
                source: SourceId::new(source),
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

    #[derive(Default)]
    struct RecordingKernel {
        lookup: RefCell<Option<CellLookup>>,
    }

    impl StorageKernel for RecordingKernel {
        fn append_cell(&mut self, _cell: StateCell) -> Result<(), KernelError> {
            Ok(())
        }

        fn lookup_cells(&self, lookup: CellLookup) -> Result<Vec<StateCell>, KernelError> {
            *self.lookup.borrow_mut() = Some(lookup);
            Ok(Vec::new())
        }
    }

    #[test]
    fn checkout_pushes_semantic_constraints_to_kernel() -> Result<(), Box<dyn std::error::Error>> {
        let kernel = RecordingKernel::default();
        checkout(
            &kernel,
            CheckoutRequest {
                scope: Some(Scope::Project("continuitydb".to_string())),
                valid_at: None,
                answerability_question: Some("what is frontier?".to_string()),
                evidence_source: Some("human".to_string()),
                minimum_confidence: Confidence::new(0.8)?,
                token_budget: 10,
            },
        )?;

        let lookup = kernel
            .lookup
            .borrow()
            .clone()
            .ok_or_else(|| std::io::Error::other("lookup was not captured"))?;
        assert_eq!(
            lookup.scope,
            Some(Scope::Project("continuitydb".to_string()))
        );
        assert_eq!(
            lookup.answerability_question,
            Some("what is frontier?".to_string())
        );
        assert_eq!(lookup.evidence_source, Some("human".to_string()));
        assert_eq!(lookup.minimum_confidence, Some(Confidence::new(0.8)?));
        Ok(())
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
                answerability_question: None,
                evidence_source: None,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 10,
            },
        )?;

        assert_eq!(slice.cells, vec![high]);
        assert_eq!(slice.total_tokens, 10);
        Ok(())
    }

    #[test]
    fn checkout_filters_by_answerability_question() -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let status = sample_cell_with_question_and_source(
            "project:continuitydb:status",
            "what is status?",
            "test",
            0.95,
            10,
        )?;
        let frontier = sample_cell_with_question_and_source(
            "project:continuitydb:frontier",
            "what is frontier?",
            "test",
            0.90,
            10,
        )?;
        kernel.append_cell(status)?;
        kernel.append_cell(frontier.clone())?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                scope: Some(Scope::Project("continuitydb".to_string())),
                valid_at: None,
                answerability_question: Some("what is frontier?".to_string()),
                evidence_source: None,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 20,
            },
        )?;

        assert_eq!(slice.cells, vec![frontier]);
        assert_eq!(slice.total_tokens, 10);
        Ok(())
    }

    #[test]
    fn checkout_filters_by_evidence_source() -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let observed = sample_cell_with_question_and_source(
            "project:continuitydb:observed",
            "what is status?",
            "sensor",
            0.95,
            10,
        )?;
        let reviewed = sample_cell_with_question_and_source(
            "project:continuitydb:reviewed",
            "what is status?",
            "human",
            0.90,
            10,
        )?;
        kernel.append_cell(observed)?;
        kernel.append_cell(reviewed.clone())?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                scope: Some(Scope::Project("continuitydb".to_string())),
                valid_at: None,
                answerability_question: None,
                evidence_source: Some("human".to_string()),
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 20,
            },
        )?;

        assert_eq!(slice.cells, vec![reviewed]);
        assert_eq!(slice.total_tokens, 10);
        Ok(())
    }

    #[test]
    fn checkout_slice_includes_citations_uncertainty_and_frontier_metadata(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let active = sample_cell("project:continuitydb:active", 0.95, 10)?;
        let mut frontier = sample_cell("project:continuitydb:frontier", 0.72, 10)?;
        frontier.activation = ActivationState::Frontier;
        kernel.append_cell(active.clone())?;
        kernel.append_cell(frontier.clone())?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                scope: Some(Scope::Project("continuitydb".to_string())),
                valid_at: None,
                answerability_question: None,
                evidence_source: None,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 20,
            },
        )?;

        assert_eq!(slice.cells, vec![active.clone(), frontier.clone()]);
        assert_eq!(slice.audit_traces[0].cell_id, active.id);
        assert_eq!(
            slice.audit_traces[0].citations,
            vec!["test://project:continuitydb:active".to_string()]
        );
        assert_eq!(slice.audit_traces[1].cell_id, frontier.id);
        assert_eq!(
            slice.audit_traces[1].citations,
            vec!["test://project:continuitydb:frontier".to_string()]
        );
        assert_eq!(slice.uncertainty[0].cell_id, active.id);
        assert_eq!(slice.uncertainty[0].max_confidence, Confidence::new(0.95)?);
        assert_eq!(slice.uncertainty[1].cell_id, frontier.id);
        assert_eq!(slice.uncertainty[1].max_confidence, Confidence::new(0.72)?);
        assert_eq!(slice.frontier_recommendations.len(), 1);
        assert_eq!(slice.frontier_recommendations[0].cell_id, frontier.id);
        assert_eq!(
            slice.frontier_recommendations[0].citations,
            vec!["test://project:continuitydb:frontier".to_string()]
        );
        Ok(())
    }

    #[test]
    fn checkout_records_token_budget_alternatives() -> Result<(), Box<dyn std::error::Error>> {
        let mut kernel = MemoryKernel::default();
        let selected = sample_cell("project:continuitydb:selected", 0.95, 10)?;
        let omitted = sample_cell("project:continuitydb:omitted", 0.90, 10)?;
        kernel.append_cell(selected.clone())?;
        kernel.append_cell(omitted.clone())?;

        let slice = checkout(
            &kernel,
            CheckoutRequest {
                scope: Some(Scope::Project("continuitydb".to_string())),
                valid_at: None,
                answerability_question: None,
                evidence_source: None,
                minimum_confidence: Confidence::new(0.7)?,
                token_budget: 10,
            },
        )?;

        assert_eq!(slice.cells, vec![selected]);
        assert_eq!(slice.alternatives.len(), 1);
        assert_eq!(slice.alternatives[0].cell_id, omitted.id);
        assert_eq!(
            slice.alternatives[0].reason,
            super::CheckoutAlternativeReason::TokenBudgetExceeded
        );
        assert_eq!(
            slice.alternatives[0].citations,
            vec!["test://project:continuitydb:omitted".to_string()]
        );
        assert_eq!(slice.alternatives[0].max_confidence, Confidence::new(0.90)?);
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
